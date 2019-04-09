#![feature(futures_api, async_await, await_macro, arbitrary_self_types)]
#![feature(nll)]
#![feature(generators)]
#![feature(never_type)]
#![deny(trivial_numeric_casts, warnings)]
#![allow(intra_doc_link_resolution_failure)]

#[macro_use]
extern crate log;

// #[macro_use]
extern crate structopt;

use std::path::PathBuf;
use structopt::StructOpt;

use app::gen::gen_invoice_id;
use app::ser_string::string_to_public_key;
use app::verify_receipt;

use stctrl::file::invoice::{load_invoice_from_file, store_invoice_to_file, Invoice};
use stctrl::file::receipt::load_receipt_from_file;

#[derive(Debug)]
enum StRegisterError {
    InvoiceFileAlreadyExists,
    StoreInvoiceError,
    LoadInvoiceError,
    LoadReceiptError,
    DestPaymentMismatch,
    InvoiceIdMismatch,
    InvalidReceipt,
    ParsePublicKeyError,
}

/// Generate invoice file
#[derive(Debug, StructOpt)]
struct GenInvoice {
    /// Amount of credits to request (Must be non-negative)
    #[structopt(short = "a")]
    amount: u128,
    /// Path of output invoice file
    #[structopt(parse(from_os_str), short = "o")]
    output: PathBuf,
}

/// Verify receipt file
#[derive(Debug, StructOpt)]
struct VerifyReceipt {
    /// Path of invoice file (Locally generated)
    #[structopt(parse(from_os_str), short = "i")]
    invoice: PathBuf,
    /// Path of receipt file (Received from buyer)
    #[structopt(parse(from_os_str), short = "r")]
    receipt: PathBuf,
    /// Public key of local seller (In base 64)
    #[structopt(short = "p")]
    seller_public_key: String,
}

#[derive(Debug, StructOpt)]
#[structopt(name = "stregister", about = "offST register")]
enum StRegister {
    #[structopt(name = "gen-invoice")]
    GenInvoice(GenInvoice),
    #[structopt(name = "verify-receipt")]
    VerifyReceipt(VerifyReceipt),
}

/// Randomly generate an invoice and store it to an output file
fn subcommand_gen_invoice(arg_gen_invoice: GenInvoice) -> Result<(), StRegisterError> {
    let invoice_id = gen_invoice_id();
    let invoice = Invoice {
        invoice_id,
        dest_payment: arg_gen_invoice.amount,
    };

    // Make sure we don't override an existing invoice file:
    if arg_gen_invoice.output.exists() {
        return Err(StRegisterError::InvoiceFileAlreadyExists);
    }

    store_invoice_to_file(&invoice, &arg_gen_invoice.output)
        .map_err(|_| StRegisterError::StoreInvoiceError)
}

/// Verify a given receipt
fn subcommand_verify_receipt(arg_verify_receipt: VerifyReceipt) -> Result<(), StRegisterError> {
    let invoice = load_invoice_from_file(&arg_verify_receipt.invoice)
        .map_err(|_| StRegisterError::LoadInvoiceError)?;

    let receipt = load_receipt_from_file(&arg_verify_receipt.receipt)
        .map_err(|_| StRegisterError::LoadReceiptError)?;

    // Make sure that the invoice and receipt files match:
    // Verify invoice_id match:
    if invoice.invoice_id != receipt.invoice_id {
        return Err(StRegisterError::InvoiceIdMismatch);
    }
    // Verify dest_payment match:
    if invoice.dest_payment != receipt.dest_payment {
        return Err(StRegisterError::DestPaymentMismatch);
    }

    let seller_public_key = string_to_public_key(&arg_verify_receipt.seller_public_key)
        .map_err(|_| StRegisterError::ParsePublicKeyError)?;

    if verify_receipt(&receipt, &seller_public_key) {
        println!("Receipt is valid!");
        Ok(())
    } else {
        Err(StRegisterError::InvalidReceipt)
    }
}

fn run() -> Result<(), StRegisterError> {
    let st_register = StRegister::from_args();
    match st_register {
        StRegister::GenInvoice(gen_invoice) => subcommand_gen_invoice(gen_invoice),
        StRegister::VerifyReceipt(verify_receipt) => subcommand_verify_receipt(verify_receipt),
    }
}

fn main() {
    if let Err(e) = run() {
        error!("error: {:?}", e);
    }
}
