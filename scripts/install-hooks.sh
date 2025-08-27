#!/bin/bash

# Pre-commit hook 설치 스크립트
echo "🔧 Installing git hooks..."

# pre-commit hook 생성
cat > .git/hooks/pre-commit << 'EOF'
#!/bin/sh

# .DS_Store 파일 체크
if git diff --cached --name-only | grep -q "\.DS_Store"; then
    echo ""
    echo "❌ ERROR: .DS_Store files detected in commit!"
    echo ""
    echo "These macOS metadata files should not be in the repository."
    echo ""
    echo "To fix this:"
    echo "  1. Remove from staging: git rm --cached .DS_Store"
    echo "  2. Delete the file: rm .DS_Store"
    echo "  3. Try committing again"
    echo ""
    exit 1
fi
EOF

# 실행 권한 부여
chmod +x .git/hooks/pre-commit

echo "✅ Git hooks installed successfully!"
echo ""
echo "The pre-commit hook will now prevent .DS_Store files from being committed."
echo "To bypass (not recommended): git commit --no-verify"