//! Output formats. Uses trait *objects* (`Box<dyn ReportFormatter>`) so the
//! format can be chosen at runtime (dynamic dispatch) – the counterpart to
//! the generics used in `service`.

use crate::model::{Money, Product};

pub trait ReportFormatter {
    fn render(&self, products: &[Product], total: Money) -> String;
}

pub struct TextReport;

impl ReportFormatter for TextReport {
    fn render(&self, products: &[Product], total: Money) -> String {
        let mut out = String::new();
        for p in products {
            out += &format!(
                "{:<8} {:<12} {:>4} x {:>8}\n",
                p.id().to_string(),
                p.name(),
                p.stock(),
                p.price().to_string()
            );
        }
        out += &format!("Total stock value: {total}\n");
        out
    }
}

pub struct CsvReport;

impl ReportFormatter for CsvReport {
    fn render(&self, products: &[Product], total: Money) -> String {
        let mut out = String::from("id,name,stock,price_cents\n");
        for p in products {
            out += &format!(
                "{},{},{},{}\n",
                p.id(),
                p.name(),
                p.stock(),
                p.price().cents()
            );
        }
        out += &format!("TOTAL,,,{}\n", total.cents());
        out
    }
}

/// Factory: picks an implementation at runtime from a string.
pub fn formatter_for(format: &str) -> Option<Box<dyn ReportFormatter>> {
    match format {
        "text" => Some(Box::new(TextReport)),
        "csv" => Some(Box::new(CsvReport)),
        _ => None,
    }
}
