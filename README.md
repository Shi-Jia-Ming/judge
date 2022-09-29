# 评测机

评测机执行来自主网站的评测命令，并返回结果

### 配置

* 需要的目录
  * `/var/judge/data` 存放持久数据
  * `/sys/fs/cgroup/judge` 评测机使用的 cgroup 树
* 权限
  * 请避免使用 root 身份运行。
