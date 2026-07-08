/*
    Webshell detection rules for source code scanning.
    Based on patterns from Neo23x0/signature-base (DRL-1.1) and community research.
    Adapted for AI agent skill artifact scanning.
*/

rule php_webshell_generic
{
    meta:
        author = "NVIDIA"
        title = "Generic PHP Webshell Detection"
        title_zh = "通用 PHP WebShell 检测"
        description = "Generic PHP webshell — eval/assert on user-controlled input"
        description_zh = "检测对用户可控输入执行 eval 或 assert 的通用 PHP WebShell"
        remediation = "Remove the webshell and audit web-accessible directories for additional payloads."
        remediation_zh = "移除 WebShell，并审计 Web 可访问目录中是否存在其他载荷。"
        classification = "harmful"
        threat_type = "GENERIC PHP WEBSHELL"
        confidence = "0.85"
        reference = "https://github.com/Neo23x0/signature-base"
        severity = "CRITICAL"
        severity_zh = "严重"
        category = "WEB_SHELL"
        category_zh = "WebShell"
    strings:
        $eval_post     = /eval\s*\(\s*\$_(POST|GET|REQUEST|COOKIE)\s*\[/ nocase
        $assert_post   = /assert\s*\(\s*\$_(POST|GET|REQUEST|COOKIE)\s*\[/ nocase
        $system_post   = /system\s*\(\s*\$_(POST|GET|REQUEST)\s*\[/ nocase
        $passthru_post = /passthru\s*\(\s*\$_(POST|GET|REQUEST)\s*\[/ nocase
        $exec_post     = /shell_exec\s*\(\s*\$_(POST|GET|REQUEST)\s*\[/ nocase
        $popen_post    = /popen\s*\(\s*\$_(POST|GET|REQUEST)\s*\[/ nocase
        $proc_open     = /proc_open\s*\(\s*\$_(POST|GET|REQUEST)\s*\[/ nocase
    condition:
        any of them
}

rule php_webshell_obfuscated
{
    meta:
        author = "NVIDIA"
        title = "Obfuscated PHP Webshell Detection"
        title_zh = "混淆 PHP WebShell 检测"
        description = "Obfuscated PHP webshell — eval(base64_decode/gzinflate/str_rot13)"
        description_zh = "检测使用 base64、gzinflate 或 str_rot13 混淆的 PHP WebShell"
        remediation = "Remove the obfuscated webshell and investigate upload or dependency compromise paths."
        remediation_zh = "移除混淆 WebShell，并调查上传入口或依赖投毒路径。"
        classification = "harmful"
        threat_type = "OBFUSCATED PHP WEBSHELL"
        confidence = "0.8"
        reference = "https://github.com/Neo23x0/signature-base"
        severity = "CRITICAL"
        severity_zh = "严重"
        category = "WEB_SHELL"
        category_zh = "WebShell"
    strings:
        $b64_eval       = /eval\s*\(\s*base64_decode\s*\(/ nocase
        $gz_eval        = /eval\s*\(\s*gzinflate\s*\(\s*base64_decode/ nocase
        $rot13_eval     = /eval\s*\(\s*str_rot13\s*\(/ nocase
        $gzuncompress   = /eval\s*\(\s*gzuncompress\s*\(/ nocase
        $preg_replace_e = /preg_replace\s*\(\s*['"]\/.*\/e['"]/ nocase
        $create_func    = /create_function\s*\(\s*['"][^'"]*['"]\s*,\s*\$/ nocase
    condition:
        any of them
}

rule php_webshell_known
{
    meta:
        author = "NVIDIA"
        title = "Known PHP Webshell Family Detection"
        title_zh = "已知 PHP WebShell 家族检测"
        description = "Known PHP webshell families (c99, r57, b374k, WSO, etc.)"
        description_zh = "检测 c99、r57、b374k、WSO 等已知 PHP WebShell 家族"
        remediation = "Remove the known webshell family artifact and investigate related compromise indicators."
        remediation_zh = "移除已知 WebShell 家族制品，并调查相关失陷指标。"
        classification = "harmful"
        threat_type = "KNOWN PHP WEBSHELL FAMILY"
        confidence = "0.9"
        reference = "https://github.com/Neo23x0/signature-base"
        severity = "CRITICAL"
        severity_zh = "严重"
        category = "WEB_SHELL"
        category_zh = "WebShell"
    strings:
        $c99      = "c99shell" nocase
        $c99v2    = "c99_sess_put" nocase
        $r57      = "r57shell" nocase
        $wso      = "Web Shell by oRb" nocase
        $wso2     = "WSO " nocase
        $b374k    = "b374k" nocase
        $alfa     = "STARTER ALFA" nocase
        $weevely  = "weevely" nocase
        $p0wny    = "p0wny" nocase
        $antsword = "antSword" nocase
        $behinder = "behinder" nocase
        $godzilla = "GodzillaShell" nocase
        $china_chopper = "China Chopper" nocase
    condition:
        any of them
}

rule python_webshell
{
    meta:
        author = "NVIDIA"
        title = "Python Webshell Detection"
        title_zh = "Python WebShell 检测"
        description = "Python webshell — exec/eval/os.popen on request input"
        description_zh = "检测对请求输入执行 exec、eval 或 os.popen 的 Python WebShell"
        remediation = "Remove the Python webshell and restrict request-controlled command execution paths."
        remediation_zh = "移除 Python WebShell，并限制请求可控的命令执行路径。"
        classification = "harmful"
        threat_type = "PYTHON WEBSHELL"
        confidence = "0.75"
        severity = "HIGH"
        severity_zh = "高危"
        category = "WEB_SHELL"
        category_zh = "WebShell"
    strings:
        $exec_request      = /exec\s*\(\s*request\./ nocase
        $eval_request      = /eval\s*\(\s*request\./ nocase
        $os_popen_request  = /os\.popen\s*\(\s*request\./ nocase
        $subprocess_req    = /subprocess\.[a-zA-Z0-9_]+\s*\(\s*request\./ nocase
        $os_system_req     = /os\.system\s*\(\s*request\./ nocase
        $flask_cmd_exec    = /os\.(system|popen)\s*\(\s*request\.(args|form|data|json)/ nocase
    condition:
        any of them
}

rule jsp_webshell
{
    meta:
        author = "NVIDIA"
        title = "JSP Webshell Detection"
        title_zh = "JSP WebShell 检测"
        description = "JSP webshell — Runtime.exec on request parameter"
        description_zh = "检测基于请求参数调用 Runtime.exec 的 JSP WebShell"
        remediation = "Remove the JSP webshell and audit deployed server-side templates."
        remediation_zh = "移除 JSP WebShell，并审计已部署的服务端模板。"
        classification = "harmful"
        threat_type = "JSP WEBSHELL"
        confidence = "0.8"
        severity = "HIGH"
        severity_zh = "高危"
        category = "WEB_SHELL"
        category_zh = "WebShell"
    strings:
        $runtime_exec    = /Runtime\.getRuntime\(\)\.exec\s*\(\s*request\.getParameter/ nocase
        $processbuilder  = /ProcessBuilder\s*\(.*request\.getParameter/ nocase
    condition:
        any of them
}

rule aspx_webshell
{
    meta:
        author = "NVIDIA"
        title = "ASPX Webshell Detection"
        title_zh = "ASPX WebShell 检测"
        description = "ASPX webshell — Process.Start on Request input"
        description_zh = "检测基于 Request 输入调用 Process.Start 的 ASPX WebShell"
        remediation = "Remove the ASPX webshell and audit deployed application directories."
        remediation_zh = "移除 ASPX WebShell，并审计已部署的应用目录。"
        classification = "harmful"
        threat_type = "ASPX WEBSHELL"
        confidence = "0.8"
        severity = "HIGH"
        severity_zh = "高危"
        category = "WEB_SHELL"
        category_zh = "WebShell"
    strings:
        $process_start = /Process\.Start\s*\(.*Request\[/ nocase
        $cmd_request   = /cmd\.exe.*Request\./ nocase
    condition:
        any of them
}
