with open('README.md', 'r') as f:
    content = f.read()

content = content.replace("- **Node.js**: `npm install @orieg/expanse`", "- **Node.js**: `npm install @orieg/expanse`\n- **Ruby**: `gem install expanse`")

with open('README.md', 'w') as f:
    f.write(content)
