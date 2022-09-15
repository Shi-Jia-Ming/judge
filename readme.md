# 评测机

评测机执行来自主网站的评测命令，并返回结果

### 配置

* 需要的目录
  * `/var/judger/data` ： 存放持久数据
  * `/tmp/judger` ：临时文件，自动创建
  * `/sys/fs/cgroup/judger` ：评测机使用的 cgroup 树
* 权限
  * 请避免使用root身份运行。
