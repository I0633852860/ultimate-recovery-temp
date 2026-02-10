import struct

with open("test_optimization.img", "wb") as f:
    # 1MB generic data
    f.write(b'\x00' * 1024 * 1024)
    
    # Text file with title, junk, size > 15KB, and Youtube link
    content = b"Title: Secret Plan\nCheck this video: https://www.youtube.com/watch?v=dQw4w9WgXcQ\n"
    content += b"A" * 20000 
    content += b"\x00\x00\x01\x02End of file."
    
    f.seek(1024 * 100)
    f.write(content)
    
    # HTML file with title and Youtube link
    html_content = b"<html><head><title>My Web Page</title></head><body>"
    html_content += b"Video: https://youtu.be/dQw4w9WgXcQ "
    html_content += b"B" * 20000 
    html_content += b"</body></html>"
    
    f.seek(1024 * 200)
    f.write(html_content)

print("Created test_optimization.img (v3)")
