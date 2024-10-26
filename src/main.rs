use std::sync::{Arc, Mutex};
use std::{env, fs, process::exit, time::Duration};

use axum::extract::State;
use axum::{routing::get, Router};
use chrono::{DateTime, Utc};
use clokwerk::Job;
use clokwerk::{Scheduler, TimeUnits};
use lettre::{
    message::header::ContentType, transport::smtp::authentication::Credentials, Message,
    SmtpTransport, Transport,
};

struct SwitchState {
    last_opened_time: Mutex<DateTime<Utc>>,
}

fn print_usage() {
    println!("Usage: rusty-switch <data.txt> <emails>");
}

fn send_checkin_email(sender: &str, recepient_emails: Vec<String>) -> Result<(), String> {
    let pw = env::var("RS_SENDER_EMAIL_PASSWORD")
        .map_err(|_| "ERROR: RS_SENDER_EMAIL_PASSWORD is not set.")?;

    for _ in recepient_emails {
        let email = Message::builder()
            .from(
                format!("Rusty Switch <{sender}>")
                    .parse()
                    .map_err(|e| format!("ERROR: From email is invalid: {e} "))?,
            )
            .reply_to(
                format!("Rusty Switch <{sender}>")
                    .parse()
                    .map_err(|e| format!("ERROR: Reply to email is invalid: {e}."))?,
            )
            .to(format!("<{sender}>")
                .parse()
                .map_err(|e| format!("ERROR: Recipient email is invalid: {e}."))?)
            .subject("Rusty Switch Check In")
            .header(ContentType::TEXT_HTML)
            .body(String::from(
                "<html><img src='http://localhost:6969/heartbeat'><h1>Checking in.</h1></html>",
            ))
            .map_err(|e| format!("ERROR: Failed to encode email body: {e}"))?;

        let creds = Credentials::new(sender.to_string(), pw.clone());

        let mailer = SmtpTransport::relay("smtp.gmail.com")
            .map_err(|e| format!("ERROR: Failed to setup SMTP relay: {e}"))?
            .credentials(creds)
            .build();

        mailer
            .send(&email)
            .map_err(|e| format!("ERROR: Failed to send email: {e}"))?;
    }

    Ok(())
}

async fn heartbeat(State(state): State<Arc<SwitchState>>) -> &'static str {
    if let Ok(mut last_opened_time) = state.last_opened_time.lock() {
        *last_opened_time = Utc::now();
        return "Heartbeat success.";
    }

    "Heartbeat failure."
}

#[tokio::main]
async fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 4 {
        eprintln!("ERROR: Did not provide enough arguments.");
        print_usage();
        exit(1);
    }

    let data_filename = args[1].to_string();
    let sender_email = args[2].to_string();
    let recepient_email = args[3].to_string();

    let data = fs::read_to_string(data_filename);

    if data.is_err() {
        eprintln!("ERROR: Invalid data.txt.");
        print_usage();
        exit(1);
    }

    if let Ok(data) = data {
        if data.is_empty() {
            eprintln!("ERROR: data file cannot be empty.");
            print_usage();
            exit(1);
        }

        let mut scheduler = Scheduler::new();
        let sender_email1 = sender_email.clone();
        let recepient_email1 = recepient_email.clone();
        scheduler.every(1.day()).at("8:00 am").run(move || {
            match send_checkin_email(&sender_email1, vec![recepient_email1.clone()]) {
                Ok(()) => (),
                Err(msg) => eprintln!("{msg}"),
            };
        });
        let _ = scheduler.watch_thread(Duration::from_millis(100));

        let shared_state = Arc::new(SwitchState {
            last_opened_time: Mutex::new(Utc::now()),
        });

        let mut scheduler = Scheduler::new();
        scheduler
            .every(7.day())
            .at("8:00 am")
            .run(|| println!("check if should send secret data now"));
        let _ = scheduler.watch_thread(Duration::from_millis(100));

        let app = Router::new()
            .route("/heartbeat", get(heartbeat))
            .with_state(shared_state);

        let listener = tokio::net::TcpListener::bind("0.0.0.0:6969").await;
        if let Ok(listener) = listener {
            println!("Running rusty-switch on 0.0.0.0:6969");
            let _ = axum::serve(listener, app).await;
        } else {
            eprintln!("ERROR: Failed to bind on port 6969.");
            exit(1);
        }
    }
}
