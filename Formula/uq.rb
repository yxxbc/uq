class Uq < Formula
  desc "Lightning-fast macOS quarantine remover & background watcher"
  homepage "https://github.com/yxxbc/uq"
  version "0.1.6"
  license "MIT"

  # Checksums are prebuilt per architecture. After publishing the tag, fill them in with:
  #   python3 scripts/sync_release_version.py --fetch-hashes
  on_macos do
    on_arm do
      url "https://github.com/yxxbc/uq/releases/download/v0.1.6/uq-v0.1.6-macos-arm64.tar.gz"
      sha256 "21f6b8e532984776bfd75831c2624fb863e124a86104fcb246096b22ebd07769"
    end

    on_intel do
      url "https://github.com/yxxbc/uq/releases/download/v0.1.6/uq-v0.1.6-macos-x86_64.tar.gz"
      sha256 "dab65497b97eadb8f596b968f4be39799daa099fed499295868abde581b94389"
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
    assert_match "uq 0.1.6", shell_output("#{bin}/uq --version")
  end
end
