"""How to handle exceptions raised by the Rust extension module `pyexc`.

Build first:   maturin develop        (inside the virtualenv)
Then run:      python python/demo.py

Every section prints what happens and asserts the expected behaviour,
so the script doubles as a test.
"""

import os
import sys
import tempfile
import traceback
import warnings

import pyexc


def section(title: str) -> None:
    print(f"\n=== {title} " + "=" * (66 - len(title)))


# =====================================================================
section("1. Basic try / except / else / finally")
# =====================================================================
for b in (4.0, 0.0):
    try:
        result = pyexc.divide(10.0, b)
    except ZeroDivisionError as e:          # catch one specific type
        print(f"  divide(10, {b}) -> caught {type(e).__name__}: {e}")
    else:                                   # runs only if nothing was raised
        print(f"  divide(10, {b}) -> {result}")
    finally:                                # runs always
        print("    (finally block ran)")


# =====================================================================
section("2. Built-in exceptions raised explicitly in Rust")
# =====================================================================
cases = [
    (pyexc.get_item, ([1, 2, 3], 10), IndexError),
    (pyexc.get_item, ([1, 2, 3], -4), IndexError),
    (pyexc.lookup, ({"a": 1}, "missing"), KeyError),
    (pyexc.ensure_str, (42,), TypeError),
    (pyexc.not_ready, (), NotImplementedError),
    (pyexc.multiply_i32, (100_000, 100_000), OverflowError),
]
for func, args, expected in cases:
    try:
        func(*args)
    except expected as e:
        call = f"{func.__name__}{args!r}"
        print(f"  {call:<34} -> {type(e).__name__}: {e}")
    else:
        raise AssertionError(f"{func.__name__} did not raise")

# KeyError stores the key in args[0], just like a Python dict:
try:
    pyexc.lookup({}, "colour")
except KeyError as e:
    assert e.args == ("colour",)
print("  get_item([1, 2, 3], -1) =", pyexc.get_item([1, 2, 3], -1))


# =====================================================================
section("3. Automatic conversion of Rust std errors (the `?` operator)")
# =====================================================================
missing = os.path.join(tempfile.gettempdir(), "pyexc-does-not-exist.txt")
cases = [
    (pyexc.parse_int, ("12x",), ValueError),          # ParseIntError
    (pyexc.parse_float, ("pi",), ValueError),         # ParseFloatError
    (pyexc.to_u8, (300,), ValueError),                # TryFromIntError
    (pyexc.read_text, (missing,), FileNotFoundError), # io::Error, NotFound
    (pyexc.read_text, (tempfile.gettempdir(),), IsADirectoryError),
    (pyexc.decode_utf8, (b"\xff\xfe abc",), UnicodeDecodeError),  # FromUtf8Error
]
for func, args, expected in cases:
    try:
        func(*args)
    except expected as e:
        print(f"  {func.__name__:<12} -> {type(e).__name__}: {e}")
    else:
        raise AssertionError(f"{func.__name__} did not raise")

# FileNotFoundError is an OSError, so the generic clause catches it as well.
# Note: PyO3 passes only the message, so e.errno is None – check the class instead.
try:
    pyexc.read_text(missing)
except OSError as e:
    print(f"  caught as OSError: {type(e).__name__}, errno={e.errno}")
    assert isinstance(e, FileNotFoundError)


# =====================================================================
section("4. Exceptions raised by PyO3 for wrong arguments")
# =====================================================================
for call, expected in [
    (lambda: pyexc.divide("ten", 2), TypeError),        # wrong type
    (lambda: pyexc.to_u8(2**70), OverflowError),         # does not fit in i64
    (lambda: pyexc.divide(1.0), TypeError),              # missing argument
    (lambda: pyexc.divide(1.0, 2.0, 3.0), TypeError),    # too many arguments
]:
    try:
        call()
    except expected as e:
        print(f"  {type(e).__name__}: {e}")


# =====================================================================
section("5. The exception hierarchy decides which except clause wins")
# =====================================================================
# LookupError  catches KeyError and IndexError,
# ArithmeticError catches ZeroDivisionError and OverflowError,
# OSError catches PermissionError, Exception catches (almost) everything.
def classify(kind: str) -> str:
    try:
        pyexc.raise_builtin(kind, "boom")
    except LookupError:
        return "LookupError branch"
    except ArithmeticError:
        return "ArithmeticError branch"
    except (TypeError, ValueError):         # tuple: either of these
        return "TypeError/ValueError branch"
    except OSError as e:
        return f"OSError branch ({type(e).__name__})"
    except Exception as e:
        return f"generic branch ({type(e).__name__})"
    return "no exception"

for kind in ["KeyError", "IndexError", "ZeroDivisionError", "OverflowError",
             "TypeError", "ValueError", "PermissionError", "RuntimeError"]:
    print(f"  {kind:<18} -> {classify(kind)}")
assert classify("IndexError") == "LookupError branch"


# =====================================================================
section("6. Custom exception hierarchy defined in Rust (create_exception!)")
# =====================================================================
print("  MRO of InsufficientFundsError:",
      " -> ".join(c.__name__ for c in pyexc.InsufficientFundsError.__mro__))
assert issubclass(pyexc.InsufficientFundsError, pyexc.BankError)
assert issubclass(pyexc.BankError, Exception)
print("  docstring:", pyexc.InsufficientFundsError.__doc__)

acc = pyexc.Account("Alice", 100.0)
attempts = [
    ("withdraw 500", lambda: acc.withdraw(500)),
    ("deposit -5", lambda: acc.deposit(-5)),
    ("deposit nan", lambda: acc.deposit(float("nan"))),
]
for label, op in attempts:
    try:
        op()
    except pyexc.InsufficientFundsError as e:   # most specific first ...
        msg, balance, requested = e.args        # extra data passed as a tuple
        print(f"  {label:<12} -> InsufficientFunds: short by {requested - balance:.2f}")
    except pyexc.BankError as e:                # ... then the base class
        print(f"  {label:<12} -> {type(e).__name__}: {e}")

acc.lock()
try:
    acc.deposit(10)
except pyexc.AccountLockedError as e:
    print(f"  locked       -> {type(e).__name__}: {e}")
acc.unlock()

# Same object as both sides: Rust's second mutable borrow fails, mapped to BankError
try:
    acc.transfer(acc, 1.0)
except pyexc.BankError as e:
    print(f"  self-transfer -> {type(e).__name__}: {e}")

# Python code can raise the Rust-defined exceptions too, and subclass them:
class OverdraftLimitError(pyexc.InsufficientFundsError):
    """A Python subclass of a Rust exception class."""

try:
    raise OverdraftLimitError("limit reached")
except pyexc.BankError as e:
    print(f"  Python subclass caught as BankError: {type(e).__name__}: {e}")


# =====================================================================
section("7. Context manager: __exit__ written in Rust gets the exception")
# =====================================================================
acc = pyexc.Account("Bob", 50.0)

# a) success -> committed
with pyexc.Transaction(acc) as a:
    a.deposit(25)
print(f"  committed:            {acc!r}")
assert acc.balance == 75.0

# b) failure in the middle -> rolled back, exception propagates
tx = pyexc.Transaction(acc)
try:
    with tx as a:
        a.withdraw(70)       # works (balance 5)
        a.withdraw(70)       # raises InsufficientFundsError
except pyexc.InsufficientFundsError:
    print(f"  rolled back + raised: {acc!r}  rolled_back={tx.rolled_back}")
assert acc.balance == 75.0 and tx.rolled_back

# c) suppress=True swallows BankErrors only ...
tx = pyexc.Transaction(acc, suppress=True)
with tx as a:
    a.withdraw(10)
    a.deposit(-1)            # InvalidAmountError -> suppressed by __exit__
print(f"  rolled back + suppressed: {acc!r}  error={tx.error!r}")
assert acc.balance == 75.0

# ... while other exceptions still propagate
try:
    with pyexc.Transaction(acc, suppress=True) as a:
        a.withdraw(10)
        {}["oops"]           # KeyError is not a BankError
except KeyError:
    print(f"  KeyError not suppressed, still rolled back: {acc!r}")
assert acc.balance == 75.0


# =====================================================================
section("8. Exception class with attributes (#[pyclass(extends=PyException)])")
# =====================================================================
try:
    pyexc.validate_user("Carol", 200, "carol@example.com")
except pyexc.ValidationError as e:
    print(f"  str : {e}")
    print(f"  repr: {e!r}")
    print(f"  field={e.field!r} reason={e.reason!r}")
    assert e.field == "age"

print("  valid:", pyexc.validate_user("Carol", 30, "carol@example.com"))

# raising it from Python works as well
try:
    raise pyexc.ValidationError("zip", "must have 5 digits")
except Exception as e:
    print(f"  raised from Python: {type(e).__name__}({e.field!r}) -> {e}")


# =====================================================================
section("9. Exception chaining (__cause__) and notes (__notes__)")
# =====================================================================
try:
    pyexc.load_config(missing)
except pyexc.ConfigError as e:
    print(f"  {type(e).__name__}: {e}")
    print(f"    __cause__ = {type(e.__cause__).__name__}: {e.__cause__}")
    print(f"    __notes__ = {e.__notes__}")
    assert isinstance(e.__cause__, FileNotFoundError)

    print("  --- the full traceback, as Python would print it ---")
    text = "".join(traceback.format_exception(e))
    print("    " + text.strip().replace("\n", "\n    "))

with tempfile.NamedTemporaryFile("w", suffix=".cfg", delete=False) as f:
    f.write("# demo\nname = demo\nthis line is broken\n")
try:
    pyexc.load_config(f.name)
except pyexc.ConfigError as e:
    print(f"  {e}  <- caused by {type(e.__cause__).__name__}: {e.__cause__}")
finally:
    os.unlink(f.name)

# Python-side chaining of a Rust exception: wrap it in your own error type
class AppError(Exception):
    pass

try:
    try:
        pyexc.parse_int("forty-two")
    except ValueError as e:
        raise AppError("invalid user input") from e
except AppError as e:
    print(f"  AppError from {type(e.__cause__).__name__}: {e.__cause__}")


# =====================================================================
section("10. ExceptionGroup and except* (several errors at once)")
# =====================================================================
users = [
    ("Dave", 40, "dave@example.com"),
    ("", 30, "nobody@example.com"),
    ("Eve", -3, "eve.example.com"),
]
try:
    pyexc.validate_users(users)
except* pyexc.ValidationError as group:     # matches every ValidationError in the group
    print(f"  {group.message}")
    for exc in group.exceptions:
        print(f"    - {exc.field:<6} {exc.reason:<35} {exc.__notes__}")
    assert len(group.exceptions) == 3

assert pyexc.validate_users(users[:1]) == 1


# =====================================================================
section("11. Python exceptions passing through Rust")
# =====================================================================
def fails_with(exc):
    def f(*_):
        raise exc
    return f

# a) Rust handles ZeroDivisionError itself (-> None) but re-raises other errors
print("  safe_call(lambda x: 1/x, 0) =", pyexc.safe_call(lambda x: 1 / x, 0))
print("  safe_call(lambda x: 1/x, 4) =", pyexc.safe_call(lambda x: 1 / x, 4))
original = KeyError("from Python")
try:
    pyexc.safe_call(fails_with(original))
except KeyError as e:
    assert e is original                    # the very same exception object
    print(f"  re-raised unchanged: {e!r}, identity kept = {e is original}")
    # the traceback shows the Python frame inside the callback
    frames = [fr.name for fr in traceback.extract_tb(e.__traceback__)]
    print(f"  traceback frames: {frames}")

# b) Rust inspects an exception and reports on it
info = pyexc.inspect_exception(lambda: [][3])
print(f"  inspect: type={info['type']} message={info['message']!r} "
      f"lookup={info['is_lookup_error']} traceback={info['has_traceback']}")
assert pyexc.inspect_exception(lambda: 42) is None

# c) retry on ConnectionError (and its subclasses), give up with a chained RuntimeError
calls = {"n": 0}
def flaky():
    calls["n"] += 1
    if calls["n"] < 3:
        raise ConnectionResetError(f"attempt {calls['n']} failed")
    return "connected"

print(f"  retry(flaky, 5) = {pyexc.retry(flaky, 5)!r} after {calls['n']} calls")
try:
    pyexc.retry(fails_with(ConnectionRefusedError("server down")), 3)
except RuntimeError as e:
    print(f"  {e}  <- cause: {type(e.__cause__).__name__}: {e.__cause__}")
try:
    pyexc.retry(fails_with(PermissionError("no access")), 3)  # not retried
except PermissionError as e:
    print(f"  not retried: PermissionError: {e}")

# d) exceptions from Python iterables / conversions inside Rust
print("  sum_numbers([1, 2.5, 3]) =", pyexc.sum_numbers([1, 2.5, 3]))
try:
    pyexc.sum_numbers([1, 2, "three"])
except TypeError as e:
    print(f"  TypeError: {e}  <- cause: {e.__cause__}")

def gen():
    yield 1
    raise ValueError("generator broke")
try:
    pyexc.sum_numbers(gen())
except ValueError as e:
    print(f"  error inside the iterator propagates: ValueError: {e}")


# =====================================================================
section("12. Exception classes from Python modules (import_exception!)")
# =====================================================================
import json

print("  parse_json_or('[1, 2]')      =", pyexc.parse_json_or("[1, 2]"))
print("  parse_json_or('{bad', 'n/a') =", pyexc.parse_json_or("{bad", "n/a"))
try:
    pyexc.strict_json_check("{'a': 1}")
except json.JSONDecodeError as e:          # raised in Rust, Python's own class
    print(f"  json.JSONDecodeError: {e}  (pos={e.pos})")


# =====================================================================
section("13. Warnings issued from Rust")
# =====================================================================
with warnings.catch_warnings(record=True) as caught:
    warnings.simplefilter("always")
    result = pyexc.old_add(2, 3)
w = caught[0]
print(f"  result={result}, warning: {w.category.__name__}: {w.message}")
print(f"  reported at {os.path.basename(w.filename)}:{w.lineno} (the caller's line)")

with warnings.catch_warnings():
    warnings.simplefilter("error")          # turn warnings into exceptions
    try:
        pyexc.old_add(2, 3)
    except DeprecationWarning as e:
        print(f"  with filter 'error' -> raised DeprecationWarning: {e}")


# =====================================================================
section("14. Rust panics -> PanicException (a BaseException!)")
# =====================================================================
# A panic means a *bug*. PanicException derives from BaseException, so a plain
# `except Exception` does NOT catch it – just like KeyboardInterrupt / SystemExit.
print("  PanicException MRO:",
      " -> ".join(c.__name__ for c in pyexc.PanicException.__mro__))
assert not issubclass(pyexc.PanicException, Exception)

for call in (lambda: pyexc.rust_panic("something went badly wrong"),
             lambda: pyexc.buggy_index([1, 2, 3], 7)):
    try:
        try:
            call()
        except Exception:
            print("  never reached")
    except pyexc.PanicException as e:
        print(f"  caught PanicException: {e}")

print("\nAll exception examples passed.")
