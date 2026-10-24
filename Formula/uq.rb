class Uq < Formula
  desc "Lightning-fast macOS quarantine remover & background watcher"
  homepage "https://github.com/yxxbc/uq"
  version "0.1.10"
  license "MIT"

  # Checksums are prebuilt per architecture. After publishing the tag, fill them in with:
  #   python3 scripts/sync_release_version.py --fetch-hashes
  on_macos do
    on_arm do
      url "https://github.com/yxxbc/uq/releases/download/v0.1.10/uq-v0.1.10-macos-arm64.tar.gz"
      sha256 "c9d3fabc09cff7897ede4b5fbd14ad6eb7dd6fe1957878966981eb69789236c6"
    end

    on_intel do
      url "https://github.com/yxxbc/uq/releases/download/v0.1.10/uq-v0.1.10-macos-x86_64.tar.gz"
      sha256 "2a4c15f88992e998580fe601eba8c5a06306378388878155422f1edb5891cc4b"
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
    assert_match "uq 0.1.10", shell_output("#{bin}/uq --version")
  end
end
