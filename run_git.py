import subprocess

def run(cmd):
    print(f"--- {cmd} ---")
    try:
        res = subprocess.run(cmd, shell=True, text=True, capture_output=True)
        print("STDOUT:", res.stdout)
        print("STDERR:", res.stderr)
    except Exception as e:
        print("ERROR:", str(e))

run("git status")
run("git diff origin/main src/contributions.rs")
run("git diff HEAD src/contributions.rs")
