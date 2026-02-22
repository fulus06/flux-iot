use crate::types::{ModbusConfig, RegisterType};
use std::net::SocketAddr;
use tokio::net::TcpStream;
use tokio_modbus::prelude::*;
use tracing::{debug, info};

/// Modbus 客户端
pub struct ModbusClient {
    config: ModbusConfig,
    context: Option<client::Context>,
}

impl ModbusClient {
    /// 创建新的 Modbus 客户端
    pub fn new(config: ModbusConfig) -> Self {
        Self {
            config,
            context: None,
        }
    }

    /// 连接到 Modbus 服务器
    pub async fn connect(&mut self) -> anyhow::Result<()> {
        let socket_addr: SocketAddr = format!("{}:{}", self.config.host, self.config.port).parse()?;
        
        let stream = TcpStream::connect(socket_addr).await?;
        let slave = Slave(self.config.slave_id);
        
        let context = client::tcp::attach_slave(stream, slave);
        self.context = Some(context);
        
        info!(
            host = %self.config.host,
            port = %self.config.port,
            slave_id = %self.config.slave_id,
            "Connected to Modbus server"
        );
        
        Ok(())
    }

    /// 断开连接
    pub async fn disconnect(&mut self) -> anyhow::Result<()> {
        self.context = None;
        debug!("Disconnected from Modbus server");
        Ok(())
    }

    /// 检查是否已连接
    pub fn is_connected(&self) -> bool {
        self.context.is_some()
    }

    /// 读取保持寄存器
    pub async fn read_holding_registers(&mut self, addr: u16, count: u16) -> anyhow::Result<Vec<u16>> {
        let ctx = self.context.as_mut()
            .ok_or_else(|| anyhow::anyhow!("Not connected"))?;
        
        let result = ctx.read_holding_registers(addr, count).await
            .map_err(|e| anyhow::anyhow!("Modbus IO error: {:?}", e))?;
        
        let values = result.map_err(|e| anyhow::anyhow!("Modbus exception: {:?}", e))?;
        
        debug!(
            addr = %addr,
            count = %count,
            "Read holding registers"
        );
        
        Ok(values)
    }

    /// 读取单个保持寄存器
    pub async fn read_holding_register(&mut self, addr: u16) -> anyhow::Result<u16> {
        let values = self.read_holding_registers(addr, 1).await?;
        Ok(values[0])
    }

    /// 写入单个保持寄存器
    pub async fn write_holding_register(&mut self, addr: u16, value: u16) -> anyhow::Result<()> {
        let ctx = self.context.as_mut()
            .ok_or_else(|| anyhow::anyhow!("Not connected"))?;
        
        ctx.write_single_register(addr, value).await
            .map_err(|e| anyhow::anyhow!("Modbus error: {:?}", e))?;
        
        debug!(
            addr = %addr,
            value = %value,
            "Wrote holding register"
        );
        
        Ok(())
    }

    /// 写入多个保持寄存器
    pub async fn write_holding_registers(&mut self, addr: u16, values: &[u16]) -> anyhow::Result<()> {
        let ctx = self.context.as_mut()
            .ok_or_else(|| anyhow::anyhow!("Not connected"))?;
        
        ctx.write_multiple_registers(addr, values).await
            .map_err(|e| anyhow::anyhow!("Modbus error: {:?}", e))?;
        
        debug!(
            addr = %addr,
            count = %values.len(),
            "Wrote multiple holding registers"
        );
        
        Ok(())
    }

    /// 读取输入寄存器
    pub async fn read_input_registers(&mut self, addr: u16, count: u16) -> anyhow::Result<Vec<u16>> {
        let ctx = self.context.as_mut()
            .ok_or_else(|| anyhow::anyhow!("Not connected"))?;
        
        let result = ctx.read_input_registers(addr, count).await
            .map_err(|e| anyhow::anyhow!("Modbus IO error: {:?}", e))?;
        
        let values = result.map_err(|e| anyhow::anyhow!("Modbus exception: {:?}", e))?;
        
        debug!(
            addr = %addr,
            count = %count,
            "Read input registers"
        );
        
        Ok(values)
    }

    /// 读取线圈
    pub async fn read_coils(&mut self, addr: u16, count: u16) -> anyhow::Result<Vec<bool>> {
        let ctx = self.context.as_mut()
            .ok_or_else(|| anyhow::anyhow!("Not connected"))?;
        
        let result = ctx.read_coils(addr, count).await
            .map_err(|e| anyhow::anyhow!("Modbus IO error: {:?}", e))?;
        
        let values = result.map_err(|e| anyhow::anyhow!("Modbus exception: {:?}", e))?;
        
        debug!(
            addr = %addr,
            count = %count,
            "Read coils"
        );
        
        Ok(values)
    }

    /// 写入单个线圈
    pub async fn write_coil(&mut self, addr: u16, value: bool) -> anyhow::Result<()> {
        let ctx = self.context.as_mut()
            .ok_or_else(|| anyhow::anyhow!("Not connected"))?;
        
        ctx.write_single_coil(addr, value).await
            .map_err(|e| anyhow::anyhow!("Modbus error: {:?}", e))?;
        
        debug!(
            addr = %addr,
            value = %value,
            "Wrote coil"
        );
        
        Ok(())
    }

    /// 读取离散输入
    pub async fn read_discrete_inputs(&mut self, addr: u16, count: u16) -> anyhow::Result<Vec<bool>> {
        let ctx = self.context.as_mut()
            .ok_or_else(|| anyhow::anyhow!("Not connected"))?;
        
        let result = ctx.read_discrete_inputs(addr, count).await
            .map_err(|e| anyhow::anyhow!("Modbus IO error: {:?}", e))?;
        
        let values = result.map_err(|e| anyhow::anyhow!("Modbus exception: {:?}", e))?;
        
        debug!(
            addr = %addr,
            count = %count,
            "Read discrete inputs"
        );
        
        Ok(values)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_modbus_client_creation() {
        let config = ModbusConfig::default();
        let client = ModbusClient::new(config);
        assert!(!client.is_connected());
    }
}
