import { Delete } from "@mui/icons-material";
import { IconButton } from "@mui/material";
import { Box } from "@mui/system";
import {
	DataGrid,
	type GridColumns,
	type GridRenderCellParams,
} from "@mui/x-data-grid";
import { order_by } from "gql";
import type { Node } from "hooks";
import { startTransition } from "react";

const MembersDataGrid = ({ node }: { node: Node }) => {
	const query = node.useQuery();
	const member = node.useMember();

	const columns: GridColumns = [
		{
			field: "name",
			headerName: "Navn",

			editable: true,
			width: 200,
		},
		{
			field: "email",
			headerName: "EMail",

			editable: true,
			width: 200,
		},
		{
			field: "hidden",
			type: "boolean",
			headerName: "Skjul",
			editable: true,
			width: 150,
		},
		{
			field: "owner",
			type: "boolean",
			headerName: "Ejer",
			editable: true,
			width: 150,
		},
		{
			field: "active",
			type: "boolean",
			editable: true,
			headerName: "Aktiv",
			width: 150,
		},
		{
			field: "actions",
			headerName: "Actions",
			width: 100,
			sortable: false,
			disableColumnMenu: true,
			renderCell: (params: GridRenderCellParams) => (
				<IconButton
					onClick={() =>
						startTransition(() => {
							member.delete(params.id.toString());
						})
					}
					size="small"
					aria-label="delete"
				>
					<Delete />
				</IconButton>
			),
		},
	];

	const handleCellEditCommit = ({
		id,
		field,
		value,
	}: {
		id: string | number;
		field: string;
		value: string;
	}) => {
		if (typeof value !== "boolean" && !["name", "email"].includes(field))
			return;
		const set = { [field]: value };
		member.update(String(id), set);
	};

	const rows = query
		?.members({ order_by: [{ user: { displayName: order_by.asc } }] })
		.map(({ id, name, email, user, owner, hidden, active, accepted }) => ({
			id,
			email: user?.email ?? email,
			name: user?.displayName ?? name,
			owner,
			hidden,
			accepted,
			active,
		}));

	if (rows === undefined || rows.length === 0 || !rows[0]?.id) return null;

	return (
		<Box sx={{ m: 0 }}>
			<DataGrid
				autoHeight
				columns={columns}
				rows={rows}
				onCellEditCommit={handleCellEditCommit}
			/>
		</Box>
	);
};

export default MembersDataGrid;
