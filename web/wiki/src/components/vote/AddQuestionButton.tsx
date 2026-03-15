import { PlusOne } from "@mui/icons-material";
import { AddContentDialog, AutoButton } from "comps";
import type { Node } from "hooks";
import { useState } from "react";
import { useTranslation } from "react-i18next";

const AddQuestionButton = ({ node }: { node: Node }) => {
	const { t } = useTranslation();
	const [open, setOpen] = useState(false);
	const query = node.useQuery();

	const handleSubmit = () => {
		setOpen(true);
	};

	if (!query?.inserts()?.some((mime) => mime.id === "vote/question"))
		return null;

	return (
		<>
			<AutoButton
				text={t("vote.question")}
				icon={<PlusOne />}
				onClick={handleSubmit}
			/>
			<AddContentDialog
				mutable={false}
				node={node}
				mimes={["vote/question"]}
				open={open}
				setOpen={setOpen}
			/>
		</>
	);
};

export default AddQuestionButton;
