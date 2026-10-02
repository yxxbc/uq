class Uq < Formula
  desc "Lightning-fast macOS quarantine remover & background watcher"
  homepage "https://github.com/yxxbc/uq"
  version "0.1.3"
  license "MIT"

  if Hardware::CPU.arm?
    url "https://github.com/yxxbc/uq/releases/download/v0.1.3/uq-v0.1.3-macos-arm64.tar.gz"
    sha256 "8e228d09995d5a6dbcddcab58b3f0935b5405f74b38669e4ff5e29440cc7f0ec"
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
    assert_match "uq 0.1.3", shell_output("#{bin}/uq --version")
  end
end
