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
    // rusty-switch data.txt email1, email2, email3
    println!("Usage: rusty-switch <data.txt> <emails>");
}

fn send_checkin_email(_sender_email: String, recepient_emails: Vec<String>) {
    let sender = env::var("RS_SENDER_EMAIL").unwrap();
    let pw = env::var("RS_SENDER_EMAIL_PASSWORD").unwrap();
    for _ in recepient_emails {
        let email = Message::builder()
            .from(format!("Rusty Switch <{sender}>").parse().unwrap())
            .reply_to(format!("Rusty Switch <{sender}>").parse().unwrap())
            .to(format!("<{sender}>").parse().unwrap())
            .subject("Rusty Switch Check In")
            .header(ContentType::TEXT_HTML)
            .body(String::from(
                "<html><img src='http://localhost:6969/heartbeat'><h1>Checking in.</h1></html>",
            ))
            .unwrap();

        let creds = Credentials::new(sender.clone(), pw.clone());

        let mailer = SmtpTransport::relay("smtp.gmail.com")
            .unwrap()
            .credentials(creds)
            .build();

        match mailer.send(&email) {
            Ok(_) => println!("Email sent successfully!"),
            Err(e) => panic!("Could not send email: {e:?}"),
        }
    }
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
        scheduler
            .every(1.day())
            .at("8:00 am")
            .run(move || send_checkin_email(sender_email1.clone(), vec![recepient_email1.clone()]));
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
