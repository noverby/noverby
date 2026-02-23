import { Delete } from "@mui/icons-material";
import { IconButton } from "@mui/material";
import { Box } from "@mui/system";
import {
	DataGrid,
	type GridColDef,
	type GridRenderCellParams,
} from "@mui/x-data-grid";
import { order_by } from "gql";
import type { Node } from "hooks";
import { startTransition } from "react";
import { useTranslation } from "react-i18next";

const MembersDataGrid = ({ node }: { node: Node }) => {
	const { t } = useTranslation();
	const query = node.useQuery();
	const member = node.useMember();

	const columns: GridColDef[] = [
		{
			field: "name",
			headerName: t("member.name"),

			editable: true,
			width: 200,
		},
		{
			field: "email",
			headerName: t("member.email"),

			editable: true,
			width: 200,
		},
		{
			field: "hidden",
			type: "boolean",
			headerName: t("member.hidden"),
			editable: true,
			width: 150,
		},
		{
			field: "owner",
			type: "boolean",
			headerName: t("member.owner"),
			editable: true,
			width: 150,
		},
		{
			field: "active",
			type: "boolean",
			editable: true,
			headerName: t("member.active"),
			width: 150,
		},
		{
			field: "actions",
			headerName: t("member.actions"),
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
					aria-label={t("common.delete").toLowerCase()}
				>
					<Delete />
				</IconButton>
			),
		},
	];

	const processRowUpdate = (
		newRow: Record<string, unknown>,
		oldRow: Record<string, unknown>,
	) => {
		// Find the changed field by comparing newRow with oldRow
		const changedField = Object.keys(newRow).find(
			(key) => newRow[key] !== oldRow[key],
		);

		if (
			changedField &&
			(typeof newRow[changedField] === "boolean" ||
				["name", "email"].includes(changedField))
		) {
			const set = { [changedField]: newRow[changedField] };
			startTransition(() => {
				member.update(String(newRow.id), set);
			});
		}

		return newRow;
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
				processRowUpdate={processRowUpdate}
			/>
		</Box>
	);
};

export default MembersDataGrid;
