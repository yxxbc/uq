class Uq < Formula
  desc "Lightning-fast macOS quarantine remover & background watcher"
  homepage "https://github.com/yxxbc/uq"
  version "0.1.0"
  license "MIT"

  if Hardware::CPU.arm?
    url "https://github.com/yxxbc/uq/releases/download/v0.1.0/uq-v0.1.0-macos-arm64.tar.gz"
    sha256 "703564637d52d5711bc070b51cf7925e0eb007e0f2f4514ba4d3cb6bc312cfa0"
  else
    url "https://github.com/yxxbc/uq.git", branch: "master"
    depends_on "rust" => :build
  end

  def install
    if Hardware::CPU.arm?
      bin.install "uq"
    else
      system "cargo", "install", *std_cargo_args
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
    assert_match "uq 0.1.0", shell_output("#{bin}/uq --version")
  end
end
