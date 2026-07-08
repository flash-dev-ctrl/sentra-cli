/*
    Hack tool and exploit kit detection rules for source code scanning.
    Based on patterns from Neo23x0/signature-base and community research.
    Detects references to known offensive tools, exploit frameworks, and
    attack utilities that should not appear in legitimate AI agent skills.
*/

rule offensive_tool_references
{
    meta:
        author = "NVIDIA"
        title = "Offensive Tool Reference Detection"
        title_zh = "进攻性工具引用检测"
        description = "References to well-known offensive security tools"
        description_zh = "检测知名进攻性安全工具引用"
        remediation = "Confirm the offensive tooling is expected for testing; remove it from production packages."
        remediation_zh = "确认进攻性工具是否仅用于授权测试；从生产环境包中移除非预期工具。"
        classification = "harmful"
        threat_type = "OFFENSIVE TOOL REFERENCE"
        confidence = "0.7"
        reference = "https://github.com/Neo23x0/signature-base"
        severity = "HIGH"
        severity_zh = "高危"
        category = "HACK_TOOL"
        category_zh = "进攻性工具"
    strings:
        $nmap_scan     = /nmap\s+-[sSUAOPpT]/ nocase
        $sqlmap        = /sqlmap.*(--url|--dbs|--dump)/ nocase
        $nikto         = /nikto\s+-h/ nocase
        $hydra         = /hydra\s+.*-[lLP]/ nocase
        $john          = /john\s+.*--wordlist/ nocase
        $hashcat       = /hashcat\s+-[mao]/ nocase
        $burpsuite     = /burpsuite|BurpCollaborator/ nocase
        $responder     = /Responder\.py/ nocase
        $bloodhound    = /SharpHound|BloodHound/ nocase
        $crackmapexec  = /crackmapexec|cme\s+smb/ nocase
        $impacket      = /impacket.*(smbclient|psexec|wmiexec|secretsdump)/ nocase
    condition:
        any of them
}

rule network_reconnaissance
{
    meta:
        author = "NVIDIA"
        title = "Network Reconnaissance Detection"
        title_zh = "网络侦察检测"
        description = "Network reconnaissance and scanning patterns"
        description_zh = "检测网络侦察和扫描模式"
        remediation = "Review scanning behavior and restrict network reconnaissance to approved security workflows."
        remediation_zh = "审查扫描行为，并将网络侦察限制在已批准的安全流程内。"
        classification = "harmful"
        threat_type = "NETWORK RECONNAISSANCE"
        confidence = "0.65"
        severity = "MEDIUM"
        severity_zh = "中危"
        category = "HACK_TOOL"
        category_zh = "进攻性工具"
    strings:
        $port_scan     = /for\s+.*\s+in\s+range\s*\(\s*\d+\s*,\s*\d{4,}\s*\).*connect/ nocase
        $masscan       = /masscan\s+.*-p/ nocase
        $arp_scan      = /arp-scan\s+--/ nocase
        $enum4linux    = /enum4linux/ nocase
        $snmp_walk     = /snmpwalk\s+-/ nocase
        $dns_enum      = /(dnsenum|dnsrecon|fierce)/ nocase
    condition:
        any of them
}

rule privilege_escalation_tools
{
    meta:
        author = "NVIDIA"
        title = "Privilege Escalation Tool Detection"
        title_zh = "权限提升工具检测"
        description = "Privilege escalation tools and techniques"
        description_zh = "检测权限提升工具和技术"
        remediation = "Remove privilege escalation tooling unless explicitly required for controlled testing."
        remediation_zh = "除受控测试明确需要外，移除权限提升工具。"
        classification = "harmful"
        threat_type = "PRIVILEGE ESCALATION TOOL"
        confidence = "0.75"
        severity = "HIGH"
        severity_zh = "高危"
        category = "HACK_TOOL"
        category_zh = "进攻性工具"
    strings:
        $linpeas       = "linpeas" nocase
        $winpeas       = "winpeas" nocase
        $pspy          = "pspy" nocase
        $linux_exploit = /(Linux_Exploit_Suggester|linux-exploit-suggester)/ nocase
        $potato        = /(JuicyPotato|RottenPotato|SweetPotato|PrintSpoofer)/ nocase
        $dirty_pipe    = "DirtyPipe" nocase
        $dirty_cow     = "dirtycow" nocase
        $suid_exploit  = /find\s+\/\s+-perm\s+-4000/ nocase
    condition:
        any of them
}

rule exploit_framework
{
    meta:
        author = "NVIDIA"
        title = "Exploit Framework Detection"
        title_zh = "漏洞利用框架检测"
        description = "Exploit framework components and payloads"
        description_zh = "检测漏洞利用框架组件和载荷"
        remediation = "Remove exploit framework components from production artifacts and verify package integrity."
        remediation_zh = "从生产制品中移除漏洞利用框架组件，并验证包完整性。"
        classification = "harmful"
        threat_type = "EXPLOIT FRAMEWORK"
        confidence = "0.8"
        severity = "HIGH"
        severity_zh = "高危"
        category = "EXPLOIT"
        category_zh = "漏洞利用"
    strings:
        $msf_payload   = /msfvenom.*-p\s+/ nocase
        $msf_console   = /msfconsole.*-x/ nocase
        $beef_hook     = /hook\.js.*BeEF/ nocase
        $set_toolkit   = /(setoolkit|Social-Engineer)/ nocase
        $pwntools      = /from\s+pwn\s+import/ nocase
        $rop_chain     = /ROP\s*\(.*elf\)/ nocase
        $shellcode_gen = /shellcode.*\\x[0-9a-f]{2}\\x[0-9a-f]{2}\\x[0-9a-f]{2}/ nocase
    condition:
        any of them
}

rule phishing_kit
{
    meta:
        author = "NVIDIA"
        title = "Phishing Kit Detection"
        title_zh = "钓鱼套件检测"
        description = "Phishing kit indicators in source code"
        description_zh = "检测源代码中的钓鱼套件指标"
        remediation = "Remove phishing kit content and investigate the source of the injected files."
        remediation_zh = "移除钓鱼套件内容，并调查注入文件的来源。"
        classification = "harmful"
        threat_type = "PHISHING KIT"
        confidence = "0.7"
        severity = "HIGH"
        severity_zh = "高危"
        category = "HACK_TOOL"
        category_zh = "进攻性工具"
    strings:
        $phish_form   = /<form.*action=.*(login|signin|verify).*method.*post/ nocase
        $cred_harvest = /(password|passwd|credential).*(file_put_contents|fwrite|>>)/ nocase
        $email_exfil  = /mail\s*\(.*(password|credential|login)/ nocase
        $telegram_bot = /api\.telegram\.org\/bot.*(password|credential|login)/ nocase
    condition:
        2 of them
}
