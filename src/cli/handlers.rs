use crate::mail::{AgentMailTransport, AgentMessage, PanoramaMail};

pub async fn send(mail: &PanoramaMail, to: &str, subject: &str, body: &str, json: bool) {
    match mail.send(to, subject, body).await {
        Ok(()) => {
            if json {
                println!("{{\"ok\":true}}");
            } else {
                println!("Sent to {to}");
            }
        }
        Err(e) => die(&e.to_string()),
    }
}

pub async fn fetch(mail: &PanoramaMail, json: bool) {
    match mail.fetch_unread().await {
        Ok(messages) => print_messages(messages, json),
        Err(e) => die(&e.to_string()),
    }
}

pub async fn search(mail: &PanoramaMail, query: &str, json: bool) {
    match mail.search(query).await {
        Ok(messages) => print_messages(messages, json),
        Err(e) => die(&e.to_string()),
    }
}

pub async fn get(mail: &PanoramaMail, uid: u32, json: bool) {
    match mail.get_by_uid(uid).await {
        Ok(Some(msg)) => print_message(&msg, json),
        Ok(None) => die(&format!("UID {} not found", uid)),
        Err(e) => die(&e.to_string()),
    }
}

pub async fn read(mail: &PanoramaMail, uid: u32) {
    match mail.mark_read(uid).await {
        Ok(()) => println!("UID {} marked as read", uid),
        Err(e) => die(&e.to_string()),
    }
}

pub async fn mailboxes(mail: &PanoramaMail, json: bool) {
    match mail.list_mailboxes().await {
        Ok(list) => {
            if json {
                println!("{}", serde_json::to_string(&list).unwrap());
            } else {
                for mb in list {
                    println!("{mb}");
                }
            }
        }
        Err(e) => die(&e.to_string()),
    }
}

fn print_messages(messages: Vec<AgentMessage>, json: bool) {
    if json {
        println!("{}", serde_json::to_string(&messages).unwrap());
        return;
    }
    if messages.is_empty() {
        println!("No messages");
        return;
    }
    for msg in &messages {
        print_message(msg, false);
        println!("---");
    }
}

fn print_message(msg: &AgentMessage, json: bool) {
    if json {
        println!("{}", serde_json::to_string(msg).unwrap());
        return;
    }
    println!("UID:     {}", msg.uid);
    println!("From:    {}", msg.from.as_deref().unwrap_or("(unknown)"));
    println!("Subject: {}", msg.subject.as_deref().unwrap_or("(no subject)"));
    if let Some(body) = &msg.body {
        let preview: String = body.chars().take(300).collect();
        println!("Body:\n{preview}");
    }
}

fn die(msg: &str) -> ! {
    eprintln!("Error: {msg}");
    std::process::exit(1);
}
