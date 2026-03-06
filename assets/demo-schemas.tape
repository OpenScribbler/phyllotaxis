# Schema exploration and example generation

Output assets/demo-schemas.gif

Set Theme "Catppuccin Mocha"
Set FontSize 16
Set Width 1200
Set Height 520
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

# List all schemas
Type "phyll --doc petstore.yaml --schemas"
Enter
Sleep 3s

# Drill into a schema
Type "phyll --doc petstore.yaml --schemas Pet"
Enter
Sleep 3s

# Generate an example
Type "phyll --doc petstore.yaml --schemas Pet --example"
Enter
Sleep 4s
