use libonm::sm::{self, Partition, PartitionKey, PartitionQoS, UFMConfig, UFMError};

#[allow(dead_code)]
pub struct UpdateOptions {
    pub pkey: String,
    pub mtu: u16,
    pub ipoib: bool,
    pub service_level: u8,
    pub rate_limit: f64,
    pub guids: Vec<String>,
}

pub async fn run(conf: UFMConfig, opt: &UpdateOptions) -> Result<(), UFMError> {
    let ufm = sm::connect(conf)?;

    let p = Partition {
        name: "".to_string(),
        pkey: PartitionKey::try_from(opt.pkey.clone())?,
        ipoib: opt.ipoib,
        qos: Some(PartitionQoS {
            mtu_limit: opt.mtu,
            service_level: opt.service_level,
            rate_limit: opt.rate_limit,
        }),
    };

    ufm.update_partition_qos(p).await?;

    Ok(())
}
