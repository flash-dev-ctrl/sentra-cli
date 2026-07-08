/*
    Crypto miner detection rules for source code scanning.
    Based on patterns from Neo23x0/signature-base and community threat intelligence.
    Covers stratum protocol, known pools, mining software references, and browser-based miners.
*/

rule crypto_stratum_protocol
{
    meta:
        author = "NVIDIA"
        title = "Cryptomining Protocol Detection"
        title_zh = "加密货币挖矿协议检测"
        description = "Stratum mining protocol usage (stratum+tcp/ssl, mining.subscribe/authorize)"
        description_zh = "检测 Stratum 挖矿协议用法"
        remediation = "Remove unauthorized mining code and investigate whether the host or package was compromised."
        remediation_zh = "移除未授权挖矿代码，并调查主机或包是否已被入侵。"
        classification = "harmful"
        threat_type = "CRYPTOMINING PROTOCOL"
        confidence = "0.9"
        reference = "https://github.com/Neo23x0/signature-base"
        severity = "HIGH"
        severity_zh = "高危"
        category = "CRYPTO_MINING"
        category_zh = "加密货币挖矿"
    strings:
        $stratum_tcp  = "stratum+tcp://" nocase
        $stratum_ssl  = "stratum+ssl://" nocase
        $mining_sub   = "mining.subscribe" nocase
        $mining_auth  = "mining.authorize" nocase
        $mining_submit = "mining.submit" nocase
    condition:
        any of them
}

rule crypto_mining_pools
{
    meta:
        author = "NVIDIA"
        title = "Cryptomining Pool Connection Detection"
        title_zh = "矿池连接检测"
        description = "Connection to known cryptocurrency mining pools"
        description_zh = "检测到已知加密货币矿池连接"
        remediation = "Block unauthorized mining pool connections and remove the code that initiates them."
        remediation_zh = "阻断未授权矿池连接，并移除发起连接的代码。"
        classification = "harmful"
        threat_type = "CRYPTOMINING POOL CONNECTION"
        confidence = "0.85"
        reference = "https://github.com/Neo23x0/signature-base"
        severity = "HIGH"
        severity_zh = "高危"
        category = "NETWORK_ACCESS"
        category_zh = "网络访问"
    strings:
        $pool_minexmr    = "pool.minexmr.com" nocase
        $pool_xmrpool    = "xmrpool.eu" nocase
        $pool_monero     = "monerohash.com" nocase
        $pool_supportxmr = "supportxmr.com" nocase
        $pool_nanopool   = "nanopool.org" nocase
        $pool_hashvault  = "hashvault.pro" nocase
        $pool_2miners    = "2miners.com" nocase
        $pool_herominers = "herominers.com" nocase
        $pool_unmine     = "unmineable.com" nocase
        $pool_nicehash   = "nicehash.com" nocase
        $pool_minergate  = "minergate.com" nocase
        $pool_f2pool     = "f2pool.com" nocase
        $pool_antpool    = "antpool.com" nocase
        $pool_viabtc     = "viabtc.com" nocase
        $pool_ethermine  = "ethermine.org" nocase
        $pool_flexpool   = "flexpool.io" nocase
        $pool_hiveon     = "hiveon.net" nocase
        $pool_ezil       = "ezil.me" nocase
    condition:
        any of them
}

rule crypto_miner_software
{
    meta:
        author = "NVIDIA"
        title = "Cryptomining Software Detection"
        title_zh = "挖矿软件检测"
        description = "References to known cryptocurrency mining software"
        description_zh = "检测已知加密货币挖矿软件引用"
        remediation = "Verify whether mining software is expected; remove it if it is not explicitly approved."
        remediation_zh = "确认挖矿软件是否符合预期；如未明确批准则移除。"
        classification = "harmful"
        threat_type = "CRYPTOMINING SOFTWARE"
        confidence = "0.8"
        reference = "https://github.com/Neo23x0/signature-base"
        severity = "HIGH"
        severity_zh = "高危"
        category = "CRYPTO_MINING"
        category_zh = "加密货币挖矿"
    strings:
        $xmrig        = "xmrig" nocase
        $xmr_stak     = "xmr-stak" nocase
        $cpuminer     = "cpuminer" nocase
        $cgminer      = "cgminer" nocase
        $bfgminer     = "bfgminer" nocase
        $ethminer     = "ethminer" nocase
        $nbminer      = "nbminer" nocase
        $phoenixminer = "phoenixminer" nocase
        $t_rex_miner  = "t-rex" nocase
        $cryptonight  = "cryptonight" nocase
        $randomx      = "randomx" nocase
    condition:
        2 of them
}

rule crypto_coinjacking
{
    meta:
        author = "NVIDIA"
        title = "Cryptojacking Script Detection"
        title_zh = "网页挖矿脚本检测"
        description = "Browser-based cryptojacking scripts (CoinHive, CryptoLoot, etc.)"
        description_zh = "检测基于浏览器的加密货币劫持脚本"
        remediation = "Remove cryptojacking scripts and review dependencies or templates that introduced them."
        remediation_zh = "移除网页挖矿脚本，并审查引入脚本的依赖或模板。"
        classification = "harmful"
        threat_type = "CRYPTOJACKING SCRIPT"
        confidence = "0.9"
        reference = "https://github.com/Neo23x0/signature-base"
        severity = "CRITICAL"
        severity_zh = "严重"
        category = "CRYPTO_MINING"
        category_zh = "加密货币挖矿"
    strings:
        $coinhive_js   = "coinhive.min.js" nocase
        $coinhive_anon = /CoinHive\.Anonymous\s*\(/ nocase
        $cryptoloot    = "cryptoloot" nocase
        $webmine_pro   = "webmine.pro" nocase
        $jsecoin       = "jsecoin" nocase
        $coin_imp      = "coin-imp" nocase
        $minero_cc     = "minero.cc" nocase
        $monerominer   = "monerominer" nocase
        $wasm_miner    = /WebAssembly\.instantiate.*(mine|hash|crypto)/
    condition:
        any of them
}
