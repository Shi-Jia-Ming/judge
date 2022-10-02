# 评测机

评测机执行来自主网站的评测命令，并返回结果

## 配置

* 需要的目录
  * `/var/judge/data` 存放持久数据
  * `/sys/fs/cgroup/judge` 评测机使用的 cgroup 树
* 权限
  * 请避免使用 root 身份运行。

## 开发状态

当前评测机的状态：

### cgroups

Cgroups 相关的限制完全不工作，使用普通用户来操作 cgroups 会比使用 root 来操作少很多重要的功能，强行避开使用 root 权限运行似乎不可取。

实验发现使用 root 用户可以限制 `cpuset cpu io memory hugetlb pids rdma misc` 等功能，但是使用普通用户则只能限制 `memory pids` 这两个功能。

### setrlimit

1.  `Resource::CPU`

    setrlimit 对于程序运行时间的限制，但是限制的只是 CPU 时间，对于阻塞调用完全不管用。

    另外理论上 setrlimit 会对超过 soft limit 的程序发送 `SIGXCPU` 信号，但是该信号可以被程序忽略并继续执行，并且当程序的 CPU 时间超出 hard limit 之后就会发送 `SIGKILL` 信号。但是经过测试发现对于 CPU 时间超出限制的程序直接返回了 SIGKILL 信号。

2.  `Resource::DATA`

    非常玄学，貌似也不是工作的很好。

以下是一些实验（均为默认的 1s, 256MB 限制）：

```cpp
#include <bits/stdc++.h>
using namespace std;
int main() {
  while(1) malloc(26214400 * 4);
}
// Some(9) SIGKILL
// Rusage: 998ms 2.445MB
```

```cpp
#include <bits/stdc++.h>
int a[268435456];
int main() {
  memset(a, 0, sizeof a);
}
// Some(11) SIGSEGV
// Rusage: 0ms 2.438MB
```

```cpp
int main() {
  return main();
}
// Some(9) SIGKILL
// Rusage: 1,091ms 2.448GB
```

```cpp
#include <chrono>
#include <thread>

int main() {
    using namespace std::this_thread;     // sleep_for, sleep_until
    using namespace std::chrono_literals; // ns, us, ms, s, h, etc.
    using std::chrono::system_clock;

    sleep_for(10s);
    sleep_until(system_clock::now() + 1s);
}
// Wrong Answer (after 11s)
// Rusage: 0ms 2.207MB
```

```cpp
#include <bits/stdc++.h>
using namespace std;
int main() {
  int a, b;
  cin >> a >> b;
  cout << a + b << endl;
  while(1);
}
// Some(9) SIGKILL
// Rusage: 0ms 4.219MB
```
