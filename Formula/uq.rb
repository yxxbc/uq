class Uq < Formula
  desc "Lightning-fast macOS quarantine remover & background watcher"
  homepage "https://github.com/SHORiN-KiWATA/uq"
  url "https://github.com/SHORiN-KiWATA/uq.git", branch: "master"
  version "0.1.0"
  license "MIT"

  head "https://github.com/SHORiN-KiWATA/uq.git", branch: "master"

  depends_on "rust" => :build

  def install
    system "cargo", "install", *std_cargo_args
  end

  def caveats
    <<~EOS
      🎉 uq 安装成功！
      建议立即运行以下命令激活开机自启后台自动解禁服务（0 内存常驻）：
        uq service install
    EOS
  end

  test do
    assert_match "uq 0.1.0", shell_output("#{bin}/uq --version")
  end
end
