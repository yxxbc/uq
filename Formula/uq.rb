class Uq < Formula
  desc "Lightning-fast macOS quarantine remover & background watcher"
  homepage "https://github.com/yxxbc/uq"
  version "0.1.4"
  license "MIT"

  if Hardware::CPU.arm?
    url "https://github.com/yxxbc/uq/releases/download/v0.1.4/uq-v0.1.4-macos-arm64.tar.gz"
    sha256 "d51c81da72ffec38413a92f7d1be12a9155126d0fc15efbc14bc933a5c081ddc"
  else
    odie "uq currently provides prebuilt macOS arm64 binaries. Intel builds are not available in this formula yet."
  end

  def install
    bin.install "uq"
  end

  def uninstall
    if (bin / "uq").exist?
      system "#{bin}/uq", "service", "uninstall"
    end
  end

  def caveats
    <<~EOS
      🎉 uq 安装成功！
      运行以下命令开启开机自启后台静默解锁服务（0 内存常驻）：
        uq service install
    EOS
  end

  test do
    assert_match "uq 0.1.4", shell_output("#{bin}/uq --version")
  end
end
