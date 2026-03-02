# 场景引擎废弃说明

> 日期: 2026-02-23
> 状态: ✅ 已完成

## 背景

场景引擎 (Scene Engine) 与规则引擎 (Rule Engine) 功能高度重叠（>90%），为了避免代码重复和维护成本，决定废弃场景引擎，统一使用功能更强大的规则引擎。

## 已删除的代码

### 1. flux-control 模块
- ✅ `crates/flux-control/src/scene/` - 整个目录已删除
  - `scene/model.rs` - 场景数据模型
  - `scene/engine.rs` - 场景执行引擎
  - `scene/trigger.rs` - 场景触发器管理
  - `scene/mod.rs` - 模块导出

### 2. flux-control-api 模块
- ✅ `crates/flux-control-api/src/handlers/scene.rs` - 场景 API 处理器已删除

### 3. 数据库迁移
- ✅ `crates/flux-control/migrations/001_create_control_tables.sql` - 已移除场景相关表
  - 移除 `scenes` 表
  - 移除 `scene_executions` 表
  - 移除相关索引和注释

### 4. 代码导出
- ✅ `crates/flux-control/src/lib.rs` - 移除场景模块导出
- ✅ `crates/flux-control-api/src/lib.rs` - 移除 `SceneAppState` 导出
- ✅ `crates/flux-control-api/src/handlers/mod.rs` - 移除场景处理器导出
- ✅ `crates/flux-control-api/src/routes.rs` - 移除 `create_scene_router` 函数

## 编译验证

所有相关包编译成功：
```bash
✅ cargo build -p flux-control
✅ cargo build -p flux-control-api
✅ cargo build -p flux-server
```

## 迁移指南

### 从场景迁移到规则

场景引擎的所有功能都可以用规则引擎实现。以下是迁移示例：

#### 示例 1: 定时场景

**原场景定义**:
```rust
Scene {
    name: "每日报告",
    triggers: vec![SceneTrigger::Schedule {
        cron: "0 8 * * *",
    }],
    action_script: r#"
        send_notification("report", "每日报告", "开始生成报告");
        log("info", "每日报告已触发");
    "#,
}
```

**迁移到规则引擎**:
```rust
Rule {
    name: "每日报告",
    trigger: RuleTrigger::Schedule {
        cron: "0 8 * * *".to_string(),
    },
    script: r#"
        send_notification("report", "每日报告", "开始生成报告");
        log("info", "每日报告已触发");
    "#,
    ..Default::default()
}
```

#### 示例 2: 设备指标触发场景

**原场景定义**:
```rust
Scene {
    name: "温度控制",
    triggers: vec![SceneTrigger::MetricChange {
        device_id: "sensor_01",
        metric: "temperature",
        operator: ComparisonOperator::GreaterThan,
        threshold: 30.0,
    }],
    action_script: r#"
        send_command("fan_01", "turn_on", #{speed: "high"});
    "#,
}
```

**迁移到规则引擎**:
```rust
Rule {
    name: "温度控制",
    trigger: RuleTrigger::DataChange {
        device_id: "sensor_01".to_string(),
        metric: Some("temperature".to_string()),
    },
    script: r#"
        let temp = get_metric("sensor_01", "temperature");
        if temp > 30.0 {
            control_device("fan_01", "turn_on", #{speed: "high"});
            log("info", "温度过高，已开启风扇");
        }
    "#,
    ..Default::default()
}
```

#### 示例 3: 设备事件触发场景

**原场景定义**:
```rust
Scene {
    name: "门禁告警",
    triggers: vec![SceneTrigger::DeviceEvent {
        device_id: "door_01",
        event_type: "unauthorized_access",
    }],
    action_script: r#"
        send_notification("alert", "安全告警", "检测到未授权访问");
        control_device("alarm_01", "turn_on", #{});
    "#,
}
```

**迁移到规则引擎**:
```rust
Rule {
    name: "门禁告警",
    trigger: RuleTrigger::DeviceEvent {
        device_id: "door_01".to_string(),
        event_type: "unauthorized_access".to_string(),
    },
    script: r#"
        send_notification("alert", "安全告警", "检测到未授权访问");
        control_device("alarm_01", "turn_on", #{});
        log("warn", "门禁告警已触发");
    "#,
    ..Default::default()
}
```

#### 示例 4: 设备状态变化触发

**原场景定义**:
```rust
Scene {
    name: "设备离线告警",
    triggers: vec![SceneTrigger::StatusChange {
        device_id: "camera_01",
        from_status: Some("Online"),
        to_status: "Offline",
    }],
    action_script: r#"
        send_notification("alert", "设备离线", "摄像头已离线");
    "#,
}
```

**迁移到规则引擎**:
```rust
Rule {
    name: "设备离线告警",
    trigger: RuleTrigger::DeviceEvent {
        device_id: "camera_01".to_string(),
        event_type: "status_change".to_string(),
    },
    script: r#"
        // 在事件处理中检查状态变化
        let status = get_device_status("camera_01");
        if status == "Offline" {
            send_notification("alert", "设备离线", "摄像头已离线");
            log("warn", "设备离线告警");
        }
    "#,
    ..Default::default()
}
```

## 规则引擎的优势

相比场景引擎，规则引擎提供了更多功能：

### 1. 限流控制
```rust
Rule {
    name: "限流告警",
    trigger: RuleTrigger::Manual,
    rate_limit: Some(RateLimit {
        max_executions: 10,
        time_window_seconds: 60,
    }),
    script: r#"
        send_notification("alert", "告警", "异常事件");
    "#,
    ..Default::default()
}
```

### 2. 优先级管理
```rust
Rule {
    name: "高优先级规则",
    priority: 90, // 1-100，数字越大优先级越高
    trigger: RuleTrigger::Manual,
    script: r#"
        log("info", "高优先级规则执行");
    "#,
    ..Default::default()
}
```

### 3. 冲突策略
```rust
Rule {
    name: "互斥规则",
    conflict_strategy: ConflictStrategy::Exclusive {
        group: "security".to_string(),
    },
    trigger: RuleTrigger::Manual,
    script: r#"
        log("info", "同组只执行一个");
    "#,
    ..Default::default()
}
```

### 4. 版本管理
```rust
Rule {
    name: "版本化规则",
    version: 2,
    previous_version: Some("rule-v1-uuid".to_string()),
    trigger: RuleTrigger::Manual,
    script: r#"
        log("info", "规则版本 2");
    "#,
    ..Default::default()
}
```

### 5. 执行统计
规则引擎自动记录：
- 执行次数
- 成功次数
- 失败次数
- 最后执行时间
- 执行历史

### 6. 更丰富的内置函数
规则引擎集成了完整的 `RuleServices`：
- ✅ 设备控制函数
- ✅ 通知函数
- ✅ 查询函数
- ✅ 工单函数
- ✅ 指标记录函数

## API 变化

### 删除的 API 端点

以下场景相关的 API 端点已删除：
- ❌ `POST /api/v1/scenes` - 创建场景
- ❌ `GET /api/v1/scenes` - 列出场景
- ❌ `GET /api/v1/scenes/:scene_id` - 获取场景详情
- ❌ `DELETE /api/v1/scenes/:scene_id` - 删除场景
- ❌ `POST /api/v1/scenes/:scene_id/execute` - 执行场景

### 使用规则 API 替代

使用规则引擎的 API（已在 flux-server 中集成）：
- ✅ `POST /api/v1/rules` - 创建规则
- ✅ `GET /api/v1/rules` - 列出规则
- ✅ `POST /api/v1/rules/reload` - 重新加载规则

## 数据库变化

### 删除的表
- ❌ `scenes` - 场景配置表
- ❌ `scene_executions` - 场景执行历史表

### 保留的表
- ✅ `device_commands` - 设备指令表
- ✅ `command_responses` - 指令响应表

规则引擎使用 `rules` 表（已在 flux-server 中创建）。

## 总结

✅ **已完成**:
1. 删除场景引擎所有代码
2. 删除场景 API 处理器
3. 更新数据库迁移文件
4. 验证编译成功
5. 提供完整的迁移指南

✅ **收益**:
1. 避免代码重复
2. 统一的自动化管理
3. 更强大的功能（限流、优先级、冲突策略等）
4. 更好的可维护性

✅ **建议**:
- 使用规则引擎实现所有自动化场景
- 参考本文档的迁移示例
- 利用规则引擎的高级功能（限流、优先级等）
