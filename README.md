# Rusty Switch

## Description
The goal of this project is to create a dead man's switch that is capable of 
sending out the data provided to it at startup to potentially multiple recipients.

It does this if the sender did not open one of the check in emails for `RS_ACTIVATION_THRESHOLD` days. 
Defaults to 7 days if unset. 

Right now we only support gmail senders but eventually we should support more. 

Expects `RS_SENDER_EMAIL_PASSWORD` env var to be set prior to launching.

## Usage

```bash
$ rusty-switch data.txt sender@gmail.com recepient1@gmail.com recepient2.gmail.com ...
```
