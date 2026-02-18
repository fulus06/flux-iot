



数据库连接池

**优化方向**（未来）:

- 根据负载动态调整连接数
- 添加连接池监控指标
- 实现连接预热机制





配置自动同步。

配置变更如何同步

> 1. **机制设计**：统一 `ConfigManager`（内部 watch channel + reload），支持 file/sqlite/postgres 三类触发方式；
> 2. **最小落地改造**：不大改你现有业务代码，只把 `AppState.config` 从“固定值”升级为“可读最新快照的句柄”（例如 `watch::Receiver<AppConfig>`），并提供一个 `state.config()` 便捷读取方法。