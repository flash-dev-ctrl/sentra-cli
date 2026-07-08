# model 子命令

sentra model

界面
左侧 agent
中间 网关
右侧 模型列表

进入 tui 界面，弹出两个选择
1. 第一个是配置模型：进入阶段二
2. 第二个是配置网关
    1. 配置网关 进入一个界面填充 url 和 key
    2. 然后携带这个填入进入阶段一

阶段一
1. 构建 ProviderData
2. 进入阶段二

阶段二
1. 收集 agent
2. 从 agent 支持获取 ProviderData
3. 进入阶段三

阶段三
1. 将 agent 与 ProviderData 组合
2. 进入阶段四

阶段四：模型切换界面
1. 左侧显示采集的 agent（provider） 列表
2. 右侧显示选中的 agent provider 的模型列表，包含统计数据，可用/总数（n/m）
3. 后台实时测试模型可用性（并发，但是要限制并发数）
4. 点击模型列表中的模型，切换模型
5. 要保持状态，不能切换到其它 provider 就丢失当前状态
6. 各个 agent（provider）独立测试，因为每个agent的探测算法不一样
7. 切换到 agent（provider） 才开始探测
8. 提示：通过易懂方式提示当前模型状态，比如：可用、不可用、测试中等


> key      ************     隐藏中间部分即可
右侧模型列表不能滚动,需要添加序号，支持在可用模型之间快速跳转，而不仅仅只能上下滚动（因为可能由几百个模型
配置模型右侧：可以有个类似广告条的状态提示，可以显示当前选中网关的完整url等
网关列表每一项, 动态刷新可用模型占比：ai-api-gateway.app.baizhi.cloud (n/m)


  Select Model and Effort
  Access legacy models by running codex -m <model_name> or in your config.toml

  1. gpt-5.5 (default)  Frontier model for complex coding, research, and real-world work.
› 2. gpt-5.4            Strong model for everyday coding.
  3. gpt-5.4-mini       Small, fast, and cost-efficient model for simpler coding tasks.
  4. gpt-5.3-codex      Coding-optimized model.
  5. gpt-5.2            Optimized for professional work and long-running agents.