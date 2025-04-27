generate_index() {
    local output_file="index.ts"
    echo "// Generated index.ts" > "$output_file"

    for file in ./*.ts; do
        filename=$(basename "$file" .ts)
        if [[ "$filename" != "index" ]]; then
            echo "export * from './$filename';" >> "$output_file"
        fi
    done
}

generate() {
    generate_index
}
