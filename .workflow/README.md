# Gitee 发行版镜像流水线

GitHub 负责构建和签名发版，Gitee 通过本流水线把已发布的附件同步到 Gitee Release。

## 配置步骤

1. 等仓库镜像把 `.workflow/GiteeRelease.yml` 和 `scripts/mirror_gitee_release.py` 同步到 Gitee 的 `main`。
2. 在 Gitee 仓库启用 Gitee Go，从仓库内的 YAML 导入这条流水线。配置由上游同步维护，不要在可视化编辑器里另存一份。
3. 在 Gitee Go 的环境变量里添加 `OPENDS_GITEE_TOKEN`（加密变量），关联到这条流水线的 Python 任务。
4. 从 `main` 手动运行一次，补发已有版本。运行环境需要 Python 3.9+、Git 和 GPG。

## 生成令牌

1. 用有 `Jac0b_Shi/OpenDS` 权限的 Gitee 账号打开「个人设置 → 安全设置 → 私人令牌」，选择「生成新令牌」。
2. 描述填 `OpenDS Gitee Release`，设置有效期。到期后在 Gitee Go 里换掉即可。
3. 权限只勾 `projects`。
4. 把生成的字符串填进 Gitee Go 的 `OPENDS_GITEE_TOKEN`。

`projects` 是 Gitee 提供的最小可用粒度，覆盖创建 Release 和上传附件所需的操作。Gitee 的令牌不区分仓库，这个令牌对其他仓库同样有效，请按普通凭据保管。

## 触发方式

推送 `v*` 标签会触发流水线。仓库镜像是否会转发同样的事件没有保证，建议在 Gitee Go 页面再加一个定时触发（例如每小时一次）作为兜底。

流水线发现新标签后，会等待对应的 GitHub Release 公开（默认最多 15 分钟），然后执行同步。GitHub 构建较慢时可以重跑。

## 流水线做什么

- 核对 GitHub 与 Gitee 的标签指向同一个提交，并验证标签签名。
- 下载 GitHub 已发布的附件，校验 SHA-256。
- 在 Gitee 创建 Release、上传附件，并下载回读确认。
- 重跑时只补传缺失的文件。

## 本地演练

不需要令牌，也不会写入 Gitee：

```powershell
python scripts/mirror_gitee_release.py --tag v0.1.0 --dry-run --wait-seconds 0
python scripts/tests/test_gitee_release.py
```

网络请求走系统代理，没有配置代理时直接连接。

## 注意事项

Gitee 的 Release API 不支持草稿。上传过程中断会在 Gitee 留下附件不全的 Release，重跑会补齐缺失文件，确认附件齐全后再对外使用。

## 参考

- [Gitee Go 触发事件](https://help.gitee.com/gitee-go/pipeline/trigger)
- [云端 Python 编译任务](https://help.gitee.com/gitee-go/plugin/ci-build)
- [流水线参数](https://help.gitee.com/gitee-go/pipeline/parameter)
- [Gitee API v5（Release / attach_files）](https://gitee.com/api/v5/swagger)
