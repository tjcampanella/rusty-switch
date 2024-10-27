use lettre::message::Mailbox;
use rand::{distributions::Alphanumeric, Rng};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::{env, fs, process::exit, time::Duration};

use axum::extract::Query;
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
    secret_token: String,
}

fn print_usage() {
    println!("Usage: rusty-switch <data.txt> <emails>");
}

fn send_checkin_email(sender: &Mailbox, secret_token: &str) -> Result<(), String> {
    let pw = env::var("RS_SENDER_EMAIL_PASSWORD")
        .map_err(|_| "ERROR: RS_SENDER_EMAIL_PASSWORD is not set.")?;
    let email = Message::builder()
            .from(sender.clone())
            .reply_to(sender.clone())
            .to(sender.clone())
            .subject("Rusty Switch Check In")
            .header(ContentType::TEXT_HTML)
            .body(format!(
                "<html><img src='http://localhost:6969/heartbeat?token={secret_token}'><h1>Checking in.</h1></html>",
            ))
            .map_err(|e| format!("ERROR: Failed to encode email body: {e}"))?;

    let creds = Credentials::new(sender.to_string(), pw);

    let mailer = SmtpTransport::relay("smtp.gmail.com")
        .map_err(|e| format!("ERROR: Failed to setup SMTP relay: {e}"))?
        .credentials(creds)
        .build();

    mailer
        .send(&email)
        .map_err(|e| format!("ERROR: Failed to send email: {e}"))?;

    Ok(())
}

fn activate_dead_man_switch(
    sender: &Mailbox,
    recipient_emails: &Vec<Mailbox>,
    data: &str,
) -> Result<(), String> {
    let pw = env::var("RS_SENDER_EMAIL_PASSWORD")
        .map_err(|_| "ERROR: RS_SENDER_EMAIL_PASSWORD is not set.")?;
    for rec in recipient_emails {
        let email = Message::builder()
            .from(sender.clone())
            .reply_to(sender.clone())
            .to(rec.clone())
            .subject("Rusty Switch ACTIVATED")
            .header(ContentType::TEXT_HTML)
            .body(format!("<html><p>{data}</p></html>",))
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

async fn heartbeat(
    State(state): State<Arc<SwitchState>>,
    Query(params): Query<HashMap<String, String>>,
) -> &'static str {
    if let Ok(mut last_opened_time) = state.last_opened_time.lock() {
        if params["token"] == state.secret_token {
            *last_opened_time = Utc::now();
            println!("Heartbeat success.");
            return "Heartbeat success.";
        }
    }

    println!("Heartbeat failure.");
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
    let sender_email_raw = args[2].to_string();
    let sender_email: Result<Mailbox, String> = format!("Rusty Switch <{sender_email_raw}>")
        .parse()
        .map_err(|_| format!("ERROR: Sender email is invalid: {sender_email_raw}"));

    if let Err(msg) = sender_email {
        eprintln!("{msg}");
        exit(1);
    }

    if let Ok(sender_email) = sender_email {
        let recipient_emails: Vec<Result<Mailbox, String>> = args[3..]
            .iter()
            .map(|email| {
                email
                    .parse()
                    .map_err(|_| format!("ERROR: Recipient email is invalid: {email}"))
            })
            .collect();

        for rec in &recipient_emails {
            if let Err(msg) = rec {
                eprintln!("{msg}");
                exit(1);
            }
        }

        let recipient_emails: Vec<Mailbox> = recipient_emails
            .iter()
            .filter_map(|rec| rec.clone().ok())
            .collect();

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

            let secret_token: String = rand::thread_rng()
                .sample_iter(&Alphanumeric)
                .take(16)
                .map(char::from)
                .collect();

            let secret_token1: String = secret_token.clone();

            println!("Starting scheduler in background thread.");
            scheduler.every(1.day()).at("8:00 am").run(move || {
                println!("Sending check in email.");
                match send_checkin_email(&sender_email1, &secret_token1) {
                    Ok(()) => (),
                    Err(msg) => eprintln!("{msg}"),
                };
            });

            let _handle = scheduler.watch_thread(Duration::from_millis(1000));

            let shared_state = Arc::new(SwitchState {
                last_opened_time: Mutex::new(Utc::now()),
                secret_token,
            });

            let shared_state1 = shared_state.clone();

            let mut scheduler = Scheduler::new();
            let threshold_raw: String =
                env::var("RS_ACTIVATION_THRESHOLD").unwrap_or_else(|_| String::from("7"));
            let mut threshold: i64 = 7;
            if let Ok(parsed) = threshold_raw.parse() {
                threshold = parsed;
            }

            scheduler.every(1.hour()).run(move || {
                let last_opened_at = &shared_state1.last_opened_time.lock();
                let curr_timestamp = Utc::now();
                if let Ok(last_opened_at) = last_opened_at {
                    let diff = curr_timestamp - **last_opened_at;

                    if diff.num_days() >= threshold {
                        println!("Dead mans switch activated!");
                        if let Err(msg) =
                            activate_dead_man_switch(&sender_email, &recipient_emails, &data)
                        {
                            eprintln!("{msg}");
                        };
                    }
                }
            });
            let _handle2 = scheduler.watch_thread(Duration::from_millis(100));

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
}





