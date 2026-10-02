class Uq < Formula
  desc "Lightning-fast macOS quarantine remover & background watcher"
  homepage "https://github.com/yxxbc/uq"
  url "https://github.com/yxxbc/uq/archive/refs/tags/v0.1.0.tar.gz"
  sha256 "bc468c1919c01a1e52e83613eaeaea0de2f3ddd5bf14798fa2bf68d9af3e5cf1"
  license "MIT"

  head "https://github.com/yxxbc/uq.git", branch: "master"

  depends_on "rust" => :build

  def install
    system "cargo", "install", *std_cargo_args
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
