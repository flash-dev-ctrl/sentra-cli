/*
 * Copyright 2026 Cisco Systems, Inc. and its affiliates
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 *
 * SPDX-License-Identifier: Apache-2.0
 */

/*
 * Detects embedded executable content in binary files within skill packages.
 * Catches ELF binaries, PE executables, Mach-O binaries, and shebang scripts
 * that may indicate supply chain compromise or hidden payloads.
 */

rule embedded_elf_binary
{
    meta:
        author = "Cisco Security"
        title = "Embedded ELF Binary Detection"
        title_zh = "嵌入 ELF 二进制文件检测"
        description = "Detects ELF executable headers embedded in skill package files"
        description_zh = "检测技能包文件中嵌入的 ELF 可执行文件头"
        remediation = "Inspect the embedded binary, verify provenance, and remove unexpected executable payloads."
        remediation_zh = "检查嵌入的二进制文件，验证来源，并移除非预期的可执行载荷。"
        classification = "harmful"
        threat_type = "supply_chain_attack"
        severity = "HIGH"
        severity_zh = "高危"
        category = "SUPPLY_CHAIN"
        category_zh = "供应链风险"
    strings:
        $elf_magic = { 7F 45 4C 46 }  // ELF magic bytes

    condition:
        $elf_magic
}

rule embedded_pe_executable
{
    meta:
        author = "Cisco Security"
        title = "Embedded PE Executable Detection"
        title_zh = "嵌入 PE 可执行文件检测"
        description = "Detects PE (Windows) executable headers embedded in skill package files"
        description_zh = "检测技能包文件中嵌入的 Windows PE 可执行文件头"
        remediation = "Inspect the embedded executable, verify provenance, and remove unexpected binary payloads."
        remediation_zh = "检查嵌入的可执行文件，验证来源，并移除非预期的二进制载荷。"
        classification = "harmful"
        threat_type = "supply_chain_attack"
        severity = "HIGH"
        severity_zh = "高危"
        category = "SUPPLY_CHAIN"
        category_zh = "供应链风险"
    strings:
        $mz_header = { 4D 5A }  // MZ header
        $pe_sig = "PE\x00\x00"

    condition:
        $mz_header at 0 and $pe_sig
}

rule embedded_macho_binary
{
    meta:
        author = "Cisco Security"
        title = "Embedded Mach-O Binary Detection"
        title_zh = "嵌入 Mach-O 二进制文件检测"
        description = "Detects Mach-O (macOS) executable headers embedded in skill package files"
        description_zh = "检测技能包文件中嵌入的 macOS Mach-O 可执行文件头"
        remediation = "Inspect the embedded Mach-O binary, verify provenance, and remove unexpected executable payloads."
        remediation_zh = "检查嵌入的 Mach-O 二进制文件，验证来源，并移除非预期的可执行载荷。"
        classification = "harmful"
        threat_type = "supply_chain_attack"
        severity = "HIGH"
        severity_zh = "高危"
        category = "SUPPLY_CHAIN"
        category_zh = "供应链风险"
    strings:
        $macho_32 = { CE FA ED FE }  // 32-bit Mach-O
        $macho_64 = { CF FA ED FE }  // 64-bit Mach-O
        $macho_fat = { CA FE BA BE }  // Universal/fat binary

    condition:
        ($macho_32 at 0) or ($macho_64 at 0) or ($macho_fat at 0)
}
