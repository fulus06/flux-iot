# 阶段 6：规则引擎 - 实施检查清单

> **日期**: 2026-02-22  
> **状态**: 📋 **实施前检查**

---

## ✅ 已确定的设计

### 1. 核心方案 ✅
- **技术选型**: 纯 Rhai 脚本引擎
- **架构**: 统一规则引擎（替代场景联动）
- **复用**: 完全复用 `flux-script` 包

### 2. 触发方式 ✅
- **手动模式**: 用户主动触发
- **定时模式**: Cron 表达式
- **条件模式**: 设备事件/数据变化

### 3. 数据模型 ✅
- `Rule`: 规则定义
- `RuleTrigger`: 触发器类型
- `RuleContext`: 执行上下文

---

## 🤔 需要补充的设计点

### 1. 规则优先级和冲突处理 ⚠️

**问题**: 多个规则同时触发如何处理？

**建议**:
```rust
pub struct Rule {
    pub priority: i32,  // 1-100，数字越大优先级越高
    pub conflict_strategy: ConflictStrategy,
}

pub enum ConflictStrategy {
    /// 并行执行（默认）
    Parallel,
    
    /// 按优先级顺序执行
    Sequential,
    
    /// 互斥执行（同组只执行一个）
    Exclusive { group: String },
}
```

**示例**:
```rust
// 高优先级规则：紧急告警
Rule {
    priority: 90,
    conflict_strategy: ConflictStrategy::Parallel,
    // ...
}

// 普通规则：温度控制
Rule {
    priority: 50,
    conflict_strategy: ConflictStrategy::Sequential,
    // ...
}
```

---

### 2. 规则执行历史和审计 ⚠️

**问题**: 如何追踪规则执行情况？

**建议**:
```rust
/// 规则执行记录
pub struct RuleExecution {
    pub id: String,
    pub rule_id: String,
    pub rule_name: String,
    pub trigger_type: String,
    pub started_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
    pub status: ExecutionStatus,
    pub error: Option<String>,
    pub context: Value,  // 执行时的上下文
}

pub enum ExecutionStatus {
    Running,
    Success,
    Failed,
    Timeout,
}
```

**用途**:
- 调试规则
- 审计追踪
- 性能分析
- 错误排查

---

### 3. 规则测试和调试模式 ⚠️

**问题**: 如何测试规则不影响实际设备？

**建议**:
```rust
pub struct Rule {
    pub test_mode: bool,  // 测试模式
}

impl RuleEngine {
    /// 测试运行规则（不实际执行动作）
    pub async fn test_rule(&self, rule_id: &str, mock_context: RuleContext) -> Result<TestResult> {
        // 执行脚本但不调用实际的设备控制函数
        // 返回执行日志和结果
    }
}

pub struct TestResult {
    pub success: bool,
    pub logs: Vec<String>,
    pub actions: Vec<String>,  // 记录会执行哪些动作
    pub error: Option<String>,
}
```

**示例**:
```rust
// 测试规则
let result = engine.test_rule("rule_001", RuleContext {
    device_data: hashmap!{
        "temperature" => 85.0.into(),
    },
    ..Default::default()
}).await?;

println!("Would execute actions: {:?}", result.actions);
```

---

### 4. 规则版本控制 ⚠️

**问题**: 规则修改后如何回滚？

**建议**:
```rust
pub struct Rule {
    pub version: i32,
    pub previous_version: Option<String>,  // 上一版本的规则 ID
}

impl RuleEngine {
    /// 保存规则新版本
    pub async fn update_rule(&self, rule: Rule) -> Result<()> {
        // 保存旧版本
        let old_rule = self.get_rule(&rule.id).await?;
        let old_version_id = format!("{}@v{}", rule.id, old_rule.version);
        self.save_rule_version(old_version_id, old_rule).await?;
        
        // 保存新版本
        let new_rule = Rule {
            version: old_rule.version + 1,
            previous_version: Some(old_version_id),
            ..rule
        };
        self.save_rule(new_rule).await?;
        
        Ok(())
    }
    
    /// 回滚到上一版本
    pub async fn rollback_rule(&self, rule_id: &str) -> Result<()> {
        // 实现回滚逻辑
    }
}
```

---

### 5. 规则执行超时控制 ⚠️

**问题**: 规则脚本执行时间过长怎么办？

**建议**:
```rust
pub struct Rule {
    pub timeout_seconds: u64,  // 默认 30 秒
}

impl RuleEngine {
    pub async fn execute_rule(&self, rule_id: &str, context: RuleContext) -> Result<()> {
        let timeout = Duration::from_secs(rule.timeout_seconds);
        
        tokio::time::timeout(timeout, async {
            // 执行规则脚本
            self.script_engine.eval(&rule.script, context).await
        }).await??;
        
        Ok(())
    }
}
```

---

### 6. 规则依赖和执行顺序 ⚠️

**问题**: 规则 A 需要在规则 B 之后执行？

**建议**:
```rust
pub struct Rule {
    pub dependencies: Vec<String>,  // 依赖的规则 ID
}

impl RuleEngine {
    pub async fn execute_rule_with_dependencies(&self, rule_id: &str) -> Result<()> {
        let rule = self.get_rule(rule_id).await?;
        
        // 先执行依赖的规则
        for dep_id in &rule.dependencies {
            self.execute_rule(dep_id, context.clone()).await?;
        }
        
        // 再执行当前规则
        self.execute_rule(rule_id, context).await?;
        
        Ok(())
    }
}
```

---

### 7. 规则分组和批量操作 ⚠️

**问题**: 如何管理大量规则？

**建议**:
```rust
pub struct Rule {
    pub group: Option<String>,  // 规则分组
    pub tags: Vec<String>,      // 标签
}

impl RuleEngine {
    /// 批量启用/禁用规则
    pub async fn enable_group(&self, group: &str, enabled: bool) -> Result<()> {
        let rules = self.find_rules_by_group(group).await?;
        for rule in rules {
            self.update_rule_status(&rule.id, enabled).await?;
        }
        Ok(())
    }
    
    /// 按标签查询规则
    pub async fn find_rules_by_tag(&self, tag: &str) -> Result<Vec<Rule>> {
        // 实现标签查询
    }
}
```

---

### 8. 规则执行限流 ⚠️

**问题**: 规则频繁触发导致系统压力？

**建议**:
```rust
pub struct Rule {
    pub rate_limit: Option<RateLimit>,
}

pub struct RateLimit {
    pub max_executions: u32,  // 最大执行次数
    pub time_window: Duration, // 时间窗口
}

impl RuleEngine {
    pub async fn execute_rule(&self, rule_id: &str, context: RuleContext) -> Result<()> {
        // 检查限流
        if let Some(limit) = &rule.rate_limit {
            let count = self.get_execution_count(rule_id, limit.time_window).await?;
            if count >= limit.max_executions {
                return Err(anyhow!("Rate limit exceeded"));
            }
        }
        
        // 执行规则
        // ...
    }
}
```

**示例**:
```rust
Rule {
    name: "高温告警",
    rate_limit: Some(RateLimit {
        max_executions: 10,
        time_window: Duration::from_secs(60),  // 1分钟最多10次
    }),
    // ...
}
```

---

### 9. 规则执行结果通知 ⚠️

**问题**: 如何知道规则执行成功或失败？

**建议**:
```rust
pub struct Rule {
    pub notification_on_success: bool,
    pub notification_on_failure: bool,
    pub notification_channels: Vec<String>,
}

impl RuleEngine {
    pub async fn execute_rule(&self, rule_id: &str, context: RuleContext) -> Result<()> {
        let result = self.run_rule_script(&rule.script, context).await;
        
        match result {
            Ok(_) if rule.notification_on_success => {
                self.send_notification(&rule, "success").await?;
            }
            Err(e) if rule.notification_on_failure => {
                self.send_notification(&rule, &format!("failed: {}", e)).await?;
            }
            _ => {}
        }
        
        result
    }
}
```

---

### 10. 规则变量和参数化 ⚠️

**问题**: 规则中的阈值等参数如何配置？

**建议**:
```rust
pub struct Rule {
    pub parameters: HashMap<String, Value>,
}
```

**示例**:
```rust
// 规则定义
Rule {
    name: "高温告警",
    parameters: hashmap!{
        "threshold" => 80.0.into(),
        "fan_speed" => "high".into(),
    },
    script: r#"
        let threshold = params.threshold;
        let temp = device.temperature;
        
        if temp > threshold {
            send_notification("urgent", "高温告警", `温度: ${temp}°C`);
            control_device("fan_001", "turn_on", #{speed: params.fan_speed});
        }
    "#,
}
```

---

## 📋 实施建议

### 必须实现（核心功能）

1. ✅ 基础规则模型
2. ✅ 三种触发方式
3. ✅ Rhai 脚本执行
4. ✅ 内置函数注册
5. ⚠️ **规则执行历史**（重要）
6. ⚠️ **规则测试模式**（重要）

### 建议实现（增强功能）

7. ⚠️ 规则优先级
8. ⚠️ 执行超时控制
9. ⚠️ 规则限流
10. ⚠️ 规则分组

### 可选实现（后续迭代）

11. ⏳ 规则版本控制
12. ⏳ 规则依赖
13. ⏳ 执行结果通知
14. ⏳ 规则参数化

---

## 🎯 最终建议

### 第一期实施（核心）

**工期**: 5 天

**功能**:
- ✅ 规则模型和存储
- ✅ 规则引擎核心
- ✅ 三种触发器
- ✅ 内置函数
- ⚠️ 执行历史
- ⚠️ 测试模式

**代码量**: ~1,800 行

---

### 第二期实施（增强）

**工期**: 2-3 天

**功能**:
- 规则优先级
- 执行超时
- 规则限流
- 规则分组

**代码量**: ~500 行

---

## ✅ 总结

### 需要补充的关键点

1. **规则执行历史** - 用于调试和审计
2. **规则测试模式** - 用于安全测试
3. **规则优先级** - 用于冲突处理
4. **执行超时控制** - 防止脚本死循环
5. **规则限流** - 防止频繁触发

### 建议

**第一期**: 实现核心功能 + 执行历史 + 测试模式  
**第二期**: 实现增强功能（优先级、超时、限流）

---

**您觉得还需要补充什么吗？或者可以开始实施了？**

**维护者**: FLUX IOT Team  
**日期**: 2026-02-22
