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
      sha256 "d0085ef301ef8340201b6187dd2a01945d2d93316672f1fc9baec337f1ac2c23"
    end

    on_intel do
      url "https://github.com/yxxbc/uq/releases/download/v0.1.10/uq-v0.1.10-macos-x86_64.tar.gz"
      sha256 "a9532b83d567326193463d67de02e5881196bb8c8850d4f3cb1d3faa2682bc8a"
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
