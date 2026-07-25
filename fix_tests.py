import os
import re

tests_dir = "src/tests"

for root, _, files in os.walk(tests_dir):
    for file in files:
        if file.endswith(".rs"):
            filepath = os.path.join(root, file)
            with open(filepath, "r") as f:
                content = f.read()

            new_content = content
            
            # Fix CreateCampaignParams
            new_content = re.sub(
                r'(max_contribution_per_user:\s*[^,}]+,?)(\s*)}',
                r'\1\n\2    token: crate::types::MaybeAddress::None,\n\2    uses_milestones: false,\n\2}',
                new_content
            )

            # Fix Campaign
            new_content = re.sub(
                r'(effective_amount_raised:\s*[^,}]+,?)(\s*)}',
                r'\1\n\2    token: soroban_sdk::Address::generate(&env),\n\2    uses_milestones: false,\n\2}',
                new_content
            )

            if new_content != content:
                with open(filepath, "w") as f:
                    f.write(new_content)
                print(f"Fixed {filepath}")
