sentra  list --format json|terminal(default) --output file

sentra list [resource]
列出已发现的 Agent 资产

sentra scan --format json|terminal(default) --output file
扫描已发现资产，或扫描指定路径

sentra model是tui模式
管理模型与 provider 配置
弹出框
1. 配置模型：给agent配置
2. 新增网关：弹出表单，填写网关信息（baseUrl + key）然后既然怒配置模型功能

先采集每个agent的provider，如果新增网关，则需要再采集后添加这个新增的provider
调用每个agent 的 getRequest获取探测的数据
应该弹出左侧agent列表，中间网关列表，右侧模型列表（需要标志模型可用状态，探测中，只有可用的可以配置）

收集所有agent的provider汇聚在一起构成Gateways列表（新增网关时，加上这个网关即可）
agent后面不要添加路径
需要获取模型列表模型列表，并发探测模型可用性
支持模型探测，模型可用性状态放在模型后面


当检测到扫描的时候带有大模型参数，但是没有配置模型，需要跳到模型tui相当于执行一次sentra model，但是列表只显示sentra网关显示（所有agent的网关，如果新增网关，也包括新增网关）


参考： E:\cw\sentra\src\cli
sentra import <files...>
导入 YARA、IOC、Hash 等规则，自动判断规则类型，规则导入接口通过lib导出放在store



sentra config [subcmd] [args...]
支持配置多个url,更新的时候复用import，自动探测（white/black hash，ti，yara）
sentra config set rule_xxx <url>
sentra config del rule_yyy <url>
sentra config set chain_key <key>
sentra config set thread_key <key>

sentra config get
查看和修改 Sentra 配置
C:\Users\23741>sentra config get

=== LLM ===
  llm.api   = https://ai-api-gateway.app.baizhi.cloud/api/openai
  llm.key   = sk-5****d2ca
  llm.model = feature/coding
  llm.protocol = responses

=== Intel ===
  (no configuration)

=== YARA Rules ===
  autonomy_abuse_generic.yara (3.9 KB)
  capability_inflation_generic.yara (3.7 KB)
  code_execution_generic.yara (3.8 KB)
  coercive_injection_generic.yara (5.3 KB)
  command_injection_generic.yara (4.1 KB)
  credential_harvesting_generic.yara (8.4 KB)
  cryptominers.yar (5.8 KB)
  embedded_binary_detection.yara (3.8 KB)
  hacktools.yar (6.5 KB)
  indirect_prompt_injection_generic.yara (3.5 KB)
  malware.yar (8.3 KB)
  prompt_injection_generic.yara (5.2 KB)
  prompt_injection_unicode_steganography.yara (3.6 KB)
  script_injection_generic.yara (4.4 KB)
  sql_injection_generic.yara (5.1 KB)
  system_manipulation_generic.yara (4.8 KB)
  tool_chaining_abuse_generic.yara (4.3 KB)
  webshells.yar (7.4 KB)

=== Threat Intelligence ===
  ti-drb_ra_c2intelfeeds-ae9c7cdc04c5.txt (12.4 KB)
  ti-emergingthreats_blockrules-3494915a7b18.txt (7.7 KB)
  ti-feodotracker_abuse_ch-3f777b871032.txt (0.6 KB)
  ti-minerstat_mining_pool_whitelist-ac448b19818a.txt (45.9 KB)
  ti-stamparm_ipsum-3a44f36041ae.txt (1938.0 KB)

=== 文件 Hash 列表 ===
  white.sha256.txt (397.5 KB)

Config: C:\Users\23741\.sentra\config.json




sentra update
再更新已配置的规则源：下载后，然后复用import

sentra skill [add|list] [source]
管理 skill 安装、删除和多 Agent 分发

