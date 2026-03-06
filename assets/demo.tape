# Phyllotaxis demo — progressive disclosure through an OpenAPI document

Output assets/demo.gif

Set Theme "Catppuccin Mocha"
Set FontSize 16
Set Width 1200
Set Height 620
Set Padding 20
Set TypingSpeed 40ms

Hide
Type "export PATH=/home/hhewett/.local/src/phyllotaxis/target/release:$PATH"
Enter
Type "cd /home/hhewett/.local/src/phyllotaxis/tests/fixtures"
Enter
Type "clear"
Enter
Show

# Level 0: Overview
Type "phyll --doc petstore.yaml"
Enter
Sleep 4s

# Level 2: Resource detail (skip listing, jump to the interesting part)
Type "phyll --doc petstore.yaml --resources pets"
Enter
Sleep 4s

# Level 3: Endpoint detail
Type "phyll --doc petstore.yaml --endpoint POST /pets"
Enter
Sleep 5s
