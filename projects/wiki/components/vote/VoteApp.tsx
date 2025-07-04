import {
	DoNotDisturb,
	HowToReg,
	HowToVote,
	Refresh,
} from "@mui/icons-material";
import {
	Avatar,
	Button,
	Card,
	CardContent,
	CardHeader,
	Checkbox,
	FormControl,
	FormControlLabel,
	FormGroup,
	FormHelperText,
	Radio,
	Stack,
	Tooltip,
} from "@mui/material";
import { useUserId } from "@nhost/nextjs";
import { HeaderCard, MimeAvatarId, MimeLoader } from "comps";
import { type Node, useSession } from "hooks";
import { useRouter } from "next/router";
import {
	type ChangeEventHandler,
	type FormEvent,
	useEffect,
	useState,
} from "react";

type Vote = boolean[];

const VoteApp = ({ node }: { node: Node }) => {
	const [session] = useSession();
	const userId = useUserId();
	const router = useRouter();
	const [refresh, setRefresh] = useState(false);
	const [loading, setLoading] = useState(false);

	const insert = node.useInsert();
	const sub = node.useSubs();
	const get = node.useSubsGet();
	const poll = get("active");

	const [helperText, setHelperText] = useState("");
	const [error, setError] = useState(false);

	const checkUnique = poll?.checkUnique({ args: { mime: "vote/vote" } });
	const canVote = !!sub?.context?.permissions({
		where: {
			_and: [
				{ mimeId: { _eq: "vote/vote" } },
				{ insert: { _eq: true } },
				{
					node: {
						members: {
							_and: [
								{
									_or: [{ nodeId: { _eq: userId } }],
								},
								{ active: { _eq: true } },
							],
						},
					},
				},
			],
		},
	})?.[0]?.id;

	const data = poll?.data();
	const options: string[] = data?.options ?? [];
	const maxVote = data?.maxVote ?? 1;
	const minVote = data?.minVote ?? 1;

	const [vote, setVote] = useState<Vote>([]);
	useEffect(() => {
		if (options?.length !== vote?.length)
			setVote(new Array(options?.length).fill(false));
	}, [JSON.stringify(options)]);

	const validate = (vote: Vote, submit: boolean) => {
		const selected = vote.filter((o) => o).length;
		// Handle blank
		if (selected === 1 && vote[vote.length - 1]) {
			return true;
		}
		if (selected > 1 && vote[vote.length - 1]) {
			setHelperText("Blank kan kun vælges alene");
			setError(true);
			return false;
		}

		if (submit && (minVote ?? 1) > selected) {
			setHelperText(
				`Vælg venligst mindst ${minVote} mulighed${
					(minVote ?? 1) > 1 ? "er" : ""
				}`,
			);
			setError(true);
			return false;
		}

		if ((maxVote ?? 1) < selected) {
			setHelperText(
				`Vælg venligst max ${maxVote} mulighed${(maxVote ?? 1) > 1 ? "er" : ""}`,
			);
			setError(true);
			return false;
		}
		return true;
	};

	const handleSubmit = async (e: FormEvent) => {
		e.preventDefault();
		setLoading(true);
		if (!validate(vote, true)) {
			return;
		}
		const name = new Date(
			Date.now() + (session?.timeDiff ?? 0),
		).toLocaleString();
		await insert({
			name,
			mimeId: "vote/vote",
			parentId: poll?.id,
			data: vote.reduce((a, e, i) => (e ? a.concat(i) : a), [] as number[]),
		});
		setLoading(false);
	};

	const handleChangeVote: ChangeEventHandler<HTMLInputElement> = (e) => {
		const voteOld =
			1 === maxVote && 1 === minVote
				? new Array(options.length).fill(false)
				: vote;
		const index = parseInt(e.target.value);
		const voteNew = [
			...voteOld.slice(0, index),
			!voteOld[index],
			...voteOld.slice(index + 1),
		];

		if (!validate(voteNew, false)) {
			return;
		}
		setVote(voteNew);

		setHelperText("");
		setError(false);
	};

	const Control = maxVote === 1 && minVote === 1 ? Radio : Checkbox;

	const status = (
		<HeaderCard
			title={canVote ? "Du har stemmeret" : "Du har ikke stemmeret"}
			subtitle={
				(poll?.mimeId === "vote/poll" &&
					canVote &&
					(checkUnique ? "Du har ikke stemt" : "Du har stemt")) ||
				""
			}
			avatar={
				<Tooltip title="Opdater status">
					<Avatar
						onClick={() => router.reload()}
						onMouseEnter={() => setRefresh(true)}
						onMouseLeave={() => setRefresh(false)}
						sx={{
							bgcolor: canVote ? "secondary.main" : "primary.main",
						}}
					>
						{refresh ? <Refresh /> : canVote ? <HowToReg /> : <DoNotDisturb />}
					</Avatar>
				</Tooltip>
			}
		/>
	);

	const pollComp = (
		<Stack spacing={1}>
			{status}
			<MimeLoader id={poll?.id} mimeId={poll?.mimeId!} />
		</Stack>
	);

	const noVoteComp = (
		<Stack spacing={1}>
			{status}
			<HeaderCard
				title="Ingen afstemning nu"
				avatar={
					<Avatar
						sx={{
							bgcolor: "secondary.main",
						}}
					>
						<DoNotDisturb />
					</Avatar>
				}
			/>
		</Stack>
	);

	const voteComp = (
		<Stack spacing={1}>
			{status}
			<Card sx={{ m: 0 }}>
				<CardHeader
					title={poll?.name}
					avatar={<MimeAvatarId id={poll?.data()?.nodeId} />}
				/>
				<CardContent>
					<form onSubmit={handleSubmit}>
						<FormControl error={error}>
							<FormGroup>
								{options?.map((opt, index: number) => (
									<FormControlLabel
										key={opt ?? index}
										value={index}
										control={
											<Control
												checked={vote[index] || false}
												onChange={handleChangeVote}
											/>
										}
										label={opt}
									/>
								))}
							</FormGroup>
							<FormHelperText>{helperText}</FormHelperText>
							<Button
								type="submit"
								variant="contained"
								color="primary"
								disabled={loading}
								endIcon={<HowToVote />}
								sx={{ m: [1, 1, 0, 0] }}
							>
								Stem
							</Button>
						</FormControl>
					</form>
				</CardContent>
			</Card>
		</Stack>
	);

	if (
		poll?.id &&
		poll?.mimeId === "vote/poll" &&
		(!poll?.mutable || checkUnique === false || canVote === false)
	)
		return pollComp;

	if (!(poll?.mutable && poll?.mimeId === "vote/poll")) return noVoteComp;

	return voteComp;
};

export default VoteApp;
