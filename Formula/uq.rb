class Uq < Formula
  desc "Lightning-fast macOS quarantine remover & background watcher"
  homepage "https://github.com/yxxbc/uq"
  version "0.1.5"
  license "MIT"

  # Checksums are prebuilt per architecture. After publishing the tag, fill them in with:
  #   python3 scripts/sync_release_version.py --fetch-hashes
  on_macos do
    on_arm do
      url "https://github.com/yxxbc/uq/releases/download/v0.1.5/uq-v0.1.5-macos-arm64.tar.gz"
      sha256 "600b79b87fabf9a436ddb7f61b0a70738156d1a11217d86210aebe3157af1467"
    end

    on_intel do
      url "https://github.com/yxxbc/uq/releases/download/v0.1.5/uq-v0.1.5-macos-x86_64.tar.gz"
      sha256 "a810f7c1d49e586ea4de09ee4a00c47ccf80edb273b50e2c0986fbd247471ee8"
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
    assert_match "uq 0.1.5", shell_output("#{bin}/uq --version")
  end
end
