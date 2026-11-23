//! Custom exception hierarchy and a context manager.
//!
//! * `create_exception!` defines new Python exception classes in Rust:
//!   ```text
//!   Exception
//!    └── BankError
//!         ├── InsufficientFundsError
//!         ├── AccountLockedError
//!         └── InvalidAmountError
//!   ```
//! * The business logic (`Ledger`) is plain Rust returning `Result<_, BankFailure>` – no
//!   Python involved, so it is unit-testable with `cargo test`.
//! * `impl From<BankFailure> for PyErr` is the bridge: `?` in a `#[pymethods]` function
//!   turns each Rust error variant into the matching Python exception.
//! * `Transaction` is a context manager whose `__exit__` receives the exception raised
//!   inside the `with` block, rolls the account back and decides whether to suppress it.

use pyo3::create_exception;
use pyo3::exceptions::PyException;
use pyo3::prelude::*;
use pyo3::types::PyType;
use std::fmt;

create_exception!(pyexc, BankError, PyException, "Base class of all banking errors.");
create_exception!(pyexc, InsufficientFundsError, BankError, "Balance too low for a withdrawal.");
create_exception!(pyexc, AccountLockedError, BankError, "The account is locked.");
create_exception!(pyexc, InvalidAmountError, BankError, "Amount is zero, negative or not finite.");

// ------------------------------------------------------------------ pure Rust part

/// An ordinary Rust error type – knows nothing about Python.
#[derive(Debug, Clone, PartialEq)]
pub enum BankFailure {
    InvalidAmount(f64),
    InsufficientFunds { balance: f64, requested: f64 },
    Locked(String),
    SameAccount,
}

impl fmt::Display for BankFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidAmount(a) => write!(f, "invalid amount {a}: must be positive and finite"),
            Self::InsufficientFunds { balance, requested } => write!(
                f,
                "cannot withdraw {requested:.2}, balance is only {balance:.2}"
            ),
            Self::Locked(owner) => write!(f, "account of {owner} is locked"),
            Self::SameAccount => write!(f, "cannot transfer money to the same account"),
        }
    }
}

impl std::error::Error for BankFailure {}

/// The bridge Rust error → Python exception (used by `?`).
impl From<BankFailure> for PyErr {
    fn from(e: BankFailure) -> PyErr {
        let msg = e.to_string();
        match e {
            BankFailure::InvalidAmount(_) => InvalidAmountError::new_err(msg),
            // An exception can carry several args: e.args == (msg, balance, requested)
            BankFailure::InsufficientFunds { balance, requested } => {
                InsufficientFundsError::new_err((msg, balance, requested))
            }
            BankFailure::Locked(_) => AccountLockedError::new_err(msg),
            BankFailure::SameAccount => BankError::new_err(msg),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Ledger {
    pub owner: String,
    pub balance: f64,
    pub locked: bool,
}

impl Ledger {
    fn check(&self, amount: f64) -> Result<(), BankFailure> {
        if !amount.is_finite() || amount <= 0.0 {
            return Err(BankFailure::InvalidAmount(amount));
        }
        if self.locked {
            return Err(BankFailure::Locked(self.owner.clone()));
        }
        Ok(())
    }

    pub fn deposit(&mut self, amount: f64) -> Result<f64, BankFailure> {
        self.check(amount)?;
        self.balance += amount;
        Ok(self.balance)
    }

    pub fn withdraw(&mut self, amount: f64) -> Result<f64, BankFailure> {
        self.check(amount)?;
        if amount > self.balance {
            return Err(BankFailure::InsufficientFunds {
                balance: self.balance,
                requested: amount,
            });
        }
        self.balance -= amount;
        Ok(self.balance)
    }
}

// ------------------------------------------------------------------ Python classes

/// A bank account. All failures raise subclasses of `BankError`.
#[pyclass(module = "pyexc")]
pub struct Account {
    pub ledger: Ledger,
}

#[pymethods]
impl Account {
    #[new]
    #[pyo3(signature = (owner, balance = 0.0))]
    fn new(owner: String, balance: f64) -> PyResult<Self> {
        if !balance.is_finite() || balance < 0.0 {
            return Err(BankFailure::InvalidAmount(balance).into());
        }
        Ok(Self {
            ledger: Ledger { owner, balance, locked: false },
        })
    }

    #[getter]
    fn owner(&self) -> &str {
        &self.ledger.owner
    }

    #[getter]
    fn balance(&self) -> f64 {
        self.ledger.balance
    }

    #[getter]
    fn locked(&self) -> bool {
        self.ledger.locked
    }

    fn lock(&mut self) {
        self.ledger.locked = true;
    }

    fn unlock(&mut self) {
        self.ledger.locked = false;
    }

    /// Returns the new balance. `?` converts `BankFailure` → Python exception.
    fn deposit(&mut self, amount: f64) -> PyResult<f64> {
        Ok(self.ledger.deposit(amount)?)
    }

    fn withdraw(&mut self, amount: f64) -> PyResult<f64> {
        Ok(self.ledger.withdraw(amount)?)
    }

    /// Transfers `amount` to `other`. If `other` is the same object, borrowing it
    /// mutably a second time fails (`PyBorrowMutError`); we turn that into a `BankError`
    /// instead of letting PyO3's generic `RuntimeError` escape.
    fn transfer(&mut self, other: &Bound<'_, Account>, amount: f64) -> PyResult<()> {
        let mut target = other
            .try_borrow_mut()
            .map_err(|_| BankFailure::SameAccount)?;
        target.ledger.check(amount)?; // target must accept deposits
        self.ledger.withdraw(amount)?;
        target.ledger.deposit(amount)?;
        Ok(())
    }

    fn __repr__(&self) -> String {
        format!(
            "Account(owner={:?}, balance={:.2}{})",
            self.ledger.owner,
            self.ledger.balance,
            if self.ledger.locked { ", locked" } else { "" }
        )
    }
}

/// `with Transaction(account) as acc:` – restores the balance if the block raises.
///
/// `__exit__(exc_type, exc_value, traceback)` gets `None, None, None` on success.
/// Returning `True` swallows the exception, `False` lets it propagate.
#[pyclass(module = "pyexc")]
pub struct Transaction {
    account: Py<Account>,
    suppress: bool,
    snapshot: Option<f64>,
    #[pyo3(get)]
    rolled_back: bool,
    #[pyo3(get)]
    error: Option<String>,
}

#[pymethods]
impl Transaction {
    /// `suppress=True` swallows `BankError`s (only those!) after the rollback.
    #[new]
    #[pyo3(signature = (account, suppress = false))]
    fn new(account: Py<Account>, suppress: bool) -> Self {
        Self { account, suppress, snapshot: None, rolled_back: false, error: None }
    }

    fn __enter__(&mut self, py: Python<'_>) -> Py<Account> {
        self.snapshot = Some(self.account.borrow(py).ledger.balance);
        self.rolled_back = false;
        self.error = None;
        self.account.clone_ref(py)
    }

    fn __exit__(
        &mut self,
        py: Python<'_>,
        exc_type: Option<&Bound<'_, PyType>>,
        exc_value: Option<&Bound<'_, PyAny>>,
        _traceback: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<bool> {
        let (Some(exc_type), Some(exc_value)) = (exc_type, exc_value) else {
            return Ok(false); // no exception: commit
        };
        if let Some(balance) = self.snapshot.take() {
            self.account.borrow_mut(py).ledger.balance = balance;
            self.rolled_back = true;
        }
        self.error = Some(format!("{}: {}", exc_type.name()?, exc_value.str()?));
        // Only swallow our own exception family; e.g. a KeyError always propagates.
        Ok(self.suppress && exc_value.is_instance_of::<BankError>())
    }
}

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    let py = m.py();
    m.add("BankError", py.get_type::<BankError>())?;
    m.add("InsufficientFundsError", py.get_type::<InsufficientFundsError>())?;
    m.add("AccountLockedError", py.get_type::<AccountLockedError>())?;
    m.add("InvalidAmountError", py.get_type::<InvalidAmountError>())?;
    m.add_class::<Account>()?;
    m.add_class::<Transaction>()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ledger(balance: f64) -> Ledger {
        Ledger { owner: "Ann".into(), balance, locked: false }
    }

    #[test]
    fn withdraw_too_much_fails() {
        assert_eq!(
            ledger(10.0).withdraw(20.0),
            Err(BankFailure::InsufficientFunds { balance: 10.0, requested: 20.0 })
        );
    }

    #[test]
    fn invalid_amounts_fail() {
        for a in [0.0, -1.0, f64::NAN, f64::INFINITY] {
            assert!(matches!(ledger(10.0).deposit(a), Err(BankFailure::InvalidAmount(_))));
        }
    }

    #[test]
    fn locked_account_fails() {
        let mut l = ledger(10.0);
        l.locked = true;
        assert_eq!(l.deposit(1.0), Err(BankFailure::Locked("Ann".into())));
    }
}
