class Sigil < Formula
  desc "Machine-first programming language designed for canonical code generation"
  homepage "https://github.com/inerte/sigil"
  version "2026-03-11T14-58-24Z"
  license "MIT"
  depends_on "node"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/inerte/sigil/releases/download/2026-03-11T14-58-24Z/sigil-2026-03-11T14-58-24Z-darwin-arm64.tar.gz"
      sha256 "1f7517f755a440c8749b246bbe0cb98c83a7aee1c59d42fffad578d57d0b6fed"
    else
      url "https://github.com/inerte/sigil/releases/download/2026-03-11T14-58-24Z/sigil-2026-03-11T14-58-24Z-darwin-x64.tar.gz"
      sha256 "633e35a31d06b36587221cff4d619caeb6692326ad0af55405644dd64267991c"
    end
  end

  on_linux do
    if Hardware::CPU.arm?
      url "https://github.com/inerte/sigil/releases/download/2026-03-11T14-58-24Z/sigil-2026-03-11T14-58-24Z-linux-arm64.tar.gz"
      sha256 "3a157f3f80fc6ded7ab2bb30538501dfc1345258077b29fabe313f062eb2cd64"
    else
      url "https://github.com/inerte/sigil/releases/download/2026-03-11T14-58-24Z/sigil-2026-03-11T14-58-24Z-linux-x64.tar.gz"
      sha256 "7c3371349bcd255226d0c4fe202327156c341b6210382eb23cf9beb64a539069"
    end
  end

  def install
    bin.install "sigil"
    pkgshare.install "README.txt"
    pkgshare.install "language"
    pkgshare.install "runtime"
  end

  test do
    assert_match "sigil 2026-03-11T14-58-24Z", shell_output("#{bin}/sigil --version")
    system bin/"sigil", "init"
    (testpath/"src/main.sigil").write <<~SIGIL
      λmain()=>Int=1+1
    SIGIL
    (testpath/"tests/basic.sigil").write <<~SIGIL
      λmain()=>Unit=()

      test "adds" {
        1+1=2
      }
    SIGIL
    system bin/"sigil", "inspect", "codegen", "src/main.sigil"
    system bin/"sigil", "compile", "."
    system bin/"sigil", "test"
  end
end
