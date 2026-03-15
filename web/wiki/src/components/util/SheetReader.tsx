import { Input } from "@mui/material";
import { type ChangeEventHandler, useId } from "react";
import { read, utils } from "xlsx";

const SheetReader = ({
	children,
	onFileLoaded,
}: {
	children: React.ReactNode;
	onFileLoaded: (data: unknown[]) => void;
}) => {
	const inputId = useId();
	const handleChangeFile: ChangeEventHandler<HTMLInputElement> = (e) => {
		const reader = new FileReader();
		const files = e.target.files;

		if (files?.length) {
			// eslint-disable-next-line functional/immutable-data
			reader.onload = () => {
				const wb = read(reader.result);
				const data = utils.sheet_to_json(wb.Sheets[wb.SheetNames[0]]);

				onFileLoaded(data ?? []);
			};

			reader.readAsArrayBuffer(files[0]);
		}
	};

	return (
		<>
			<Input
				id={inputId}
				type="file"
				onChange={handleChangeFile}
				sx={{ display: "none" }}
			/>
			<label htmlFor={inputId}>{children}</label>
		</>
	);
};

export default SheetReader;
