import { Dialog } from "./Dialog";

// PRD §6.4：网络活动说明页，教用户自行验证文件不会外发。
export function NetworkPanel({ onClose }: { onClose: () => void }) {
  return (
    <Dialog title="网络活动" onClose={onClose}>
      <p className="dialog-lead">除首次激活外，本应用不会发起任何网络请求。</p>
      <ul className="bullet-list">
        <li>文件不会发送到任何服务器，合并、拆分、压缩、转换、OCR、签名与批量全部在本机完成。</li>
        <li>没有账号体系，也没有服务端；打开即用。</li>
        <li>不收集使用数据；本地日志默认关闭，开启后只记录操作类型与错误码。</li>
        <li>激活只发送密钥与机器指纹哈希，不与任何文件信息关联。</li>
      </ul>

      <h3 className="dialog-section-title">如何自行验证</h3>
      <ol className="bullet-list ordered">
        <li>打开系统网络监控或抓包工具（如 Wireshark、Fiddler、Little Snitch）。</li>
        <li>按进程过滤 PayOnce PDF。</li>
        <li>执行一次任意处理任务，观察是否有文件数据流向外部地址。</li>
        <li>断开网络后重复操作，功能应照常可用（首次激活除外）。</li>
      </ol>
    </Dialog>
  );
}
