# model 子命令

sentra model

支持 CLI 配置，也可以进入 TUI 交互界面

进入 TUI 后，先发现所有 agent 并收集所有 provider（复用已有代码），并显示支持配置 Provider 的 agent

选择一个 agent 后，进入模型配置界面

左边显示采集到的所有 provider 包括其它 agent 的，右边显示该 provider 的模型列表
该界面支持新增 provider，添加网关与Provider同级，点击后弹出对话框，填入 api 和 key 即可


支持 ESC 返回上一级
在模型配置界面，要高亮标明 agent 已配置的 provider，如果没有，则不用
同时状态栏要显示光标所在模型的 api 和脱敏的 key

# bug
1. 光标选中需要改变背景色，现在只改变了字体颜色，并不能快速捕获用户视觉焦点
2. sentra model 太慢了（修改sentra lib，给发现agent添加参数跳过安装探测，默认不跳过，进程探测只对又env的entry才生效），并且更新 provider 也很慢，我认为就是备份然后写入，应该一两秒就行
3. 文字太乱了，需要美化一下，确保布局工整，尤其是Providers列表
4. 随便添加的provider，models竟然有列表
5. 界面没有显示当前正在配置的agent
