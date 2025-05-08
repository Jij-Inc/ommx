import glob
import os

target_pattern = "**/*_pb2.pyi"  # 検索するファイルのパターン (サブディレクトリも含む)
string_to_insert = "# pyright: reportIncompatibleVariableOverride=false"
line_to_insert = string_to_insert + "\n"  # 改行コードを追加

# カレントディレクトリを起点にする場合
# base_dir = "."
# 特定のディレクトリを起点にする場合 (例: "project_root/protos")
base_dir = "."  # 必要に応じて変更してください

# glob.glob() で recursive=True を使うと ** でサブディレクトリを検索できます
# os.path.join でパスを結合するとOS間の互換性が保てます
for filepath in glob.glob(os.path.join(base_dir, target_pattern), recursive=True):
    if not os.path.isfile(filepath):  # 念のためファイルであるか確認
        continue

    try:
        with open(filepath, "r", encoding="utf-8") as f_read:
            original_content = f_read.readlines()

        # 既に挿入済みかチェック (任意)
        if original_content and original_content[0].strip() == string_to_insert:
            print(f"Skipped (already inserted): {filepath}")
            continue

        with open(filepath, "w", encoding="utf-8") as f_write:
            f_write.write(line_to_insert)
            f_write.writelines(original_content)
        print(f"Inserted line into: {filepath}")

    except Exception as e:
        print(f"Error processing file {filepath}: {e}")

print("Processing complete.")
