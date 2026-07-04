import mordant

doc = mordant.parse("# Hi\n\n**world** with a [link](http://example.com)\n")
assert doc.source.startswith("# Hi"), "source getter failed"
assert doc.children[0].kind == "Heading", f"expected Heading, got {doc.children[0].kind}"

for node in doc.walk("depth"):
    _ = node.text
    _ = node.parent

print("ok")
