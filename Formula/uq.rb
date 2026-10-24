class Uq < Formula
  desc "Lightning-fast macOS quarantine remover & background watcher"
  homepage "https://github.com/yxxbc/uq"
  version "0.1.9"
  license "MIT"

  # Checksums are prebuilt per architecture. After publishing the tag, fill them in with:
  #   python3 scripts/sync_release_version.py --fetch-hashes
  on_macos do
    on_arm do
      url "https://github.com/yxxbc/uq/releases/download/v0.1.9/uq-v0.1.9-macos-arm64.tar.gz"
      sha256 "d44c534d87d264cd5a3a5890b88155b780fd9d0a3173a7fcb2210d3580082f5c"
    end

    on_intel do
      url "https://github.com/yxxbc/uq/releases/download/v0.1.9/uq-v0.1.9-macos-x86_64.tar.gz"
      sha256 "34474862a1901029e5c18c45663d929c458f8bcd59fe2665cf0b011fac3935f1"
    end
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
      运行以下命令开启开机自启后台实时解锁服务（FSEvents，毫秒级触发）：
        uq service install
      需要一次性手动清理时运行：uq do
    EOS
  end

  test do
    assert_match "uq 0.1.9", shell_output("#{bin}/uq --version")
  end
end
