use libonm::xpu::XPUError;

use crate::{
    list,
    types::{Context, BMC},
};

fn get_bmc(ctx: &Context, xpu: usize) -> Result<&BMC, XPUError> {
    ctx.bmc
        .get(xpu)
        .ok_or_else(|| XPUError::NotFound(format!("XPU index {xpu}")))
}

pub async fn run(ctx: &Context, xpu: usize) -> Result<(), XPUError> {
    let bmc = get_bmc(ctx, xpu)?;
    let result = list::list_bmc(bmc, &ctx.username, &ctx.password, ctx.tls_verify).await?;

    list::print_header();
    list::print_result(&result);

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn context() -> Context {
        Context {
            username: "user".to_string(),
            password: "password".to_string(),
            tls_verify: true,
            bmc: vec![BMC {
                name: "xpu-0".to_string(),
                vendor: "bluefield".to_string(),
                address: "https://127.0.0.1".to_string(),
                username: None,
                password: None,
                tls_verify: None,
            }],
        }
    }

    #[test]
    fn selects_xpu_by_configuration_index() {
        assert_eq!(get_bmc(&context(), 0).unwrap().name, "xpu-0");
    }

    #[test]
    fn rejects_out_of_bounds_index() {
        assert!(matches!(get_bmc(&context(), 1), Err(XPUError::NotFound(_))));
    }
}
