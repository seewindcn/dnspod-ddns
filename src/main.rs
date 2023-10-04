#![feature(async_closure)]
use crate::dnspod_api::{DnspodApi, Record};

use std::error;
use anyhow::Error;
use chrono::Local;
use clap::Parser;
use log::{error, info, trace, warn};
use std::io::Write;
use tokio::time::{self, Duration};

mod args;
mod dnspod_api;

async fn get_ip() -> Result<String, Error> {
    let result = reqwest::get("http://ns1.dnspod.net:6666")
        .await?
        .text()
        .await?;

    let ip = result.trim().to_owned();
    info!("get_my_ip: {}", ip);
    Ok(ip)
}

async fn get_record(sub_domain: &str, api: &DnspodApi) -> Result<Record, Error> {
    let record = api.get_record(sub_domain).await?;
    match record {
        Record::NotFound => {
            error!("record not found");
            return Err(anyhow::anyhow!("record({}) not found", sub_domain))
        }
        _ => {
            info!("get_record: {} -> {}", sub_domain, record);
            return Ok(record);
        }
    }
}

async fn get_record_id_value(sub_domain: &str, api: &DnspodApi) -> Result<(String, String), Error> {
    let record = get_record(sub_domain, api).await?;
    match record {
        Record::A(id, value) => {
            return Ok((id, value));
        }
        _ => {
            return Ok(("".to_string(), "".to_string()));
        }
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), anyhow::Error>{
    let args = args::Args::parse();

    let mut level = log::LevelFilter::Info;

    if args.verbose {
        level = log::LevelFilter::Trace;
    }

    env_logger::Builder::new()
        .format(|buf, record| {
            writeln!(
                buf,
                "{} {} {}",
                Local::now().format("%Y-%m-%dT%H:%M:%S%.3f"),
                record.level(),
                record.args()
            )
        })
        .filter_module("dnspod_ddns", level)
        .init();

    let api = DnspodApi::new(args.token, args.domain);
    let mut interval = time::interval(Duration::from_secs(args.interval));
    let mut record_id;
    let mut record_value;
    (record_id, record_value) = get_record_id_value(&args.sub_domain, &api).await?;
    let mut times = 0;

    loop {
        interval.tick().await;

        let my_ip = get_ip().await;

        match my_ip {
            Ok(my_ip) => {
                if my_ip != record_value {
                    info!("update record, {}", my_ip);
                    let result = api
                        .update_record(&args.sub_domain, &record_id, &my_ip)
                        .await;
                    match result {
                        Ok(_) => {
                            info!("update record success");
                            record_value = my_ip;
                        }
                        Err(e) => {
                            warn!("update record failed, {}", e);
                        }
                    }
                } else {
                    trace!("ip not changed");
                }
            }
            Err(e) => {
                warn!("get my ip: {:?}", e);
            }
        }

        if times >= args.refresh {
            (record_id, record_value) = get_record_id_value(&args.sub_domain, &api).await?;
            times = 0;
        }
        times += 1;
    }
}

#[cfg(test)]
mod test {
    use crate::get_ip;

    #[tokio::test]
    async fn test_get_ip() {
        let ip = get_ip().await;
        assert!(ip.is_ok());
        println!("ip: {:?}", ip.unwrap());
    }
}
