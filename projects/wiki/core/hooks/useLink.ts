import { fromId } from "core/path";
import { resolve } from "gql";
import { usePath } from "hooks";
import { useNavigate } from "react-router-dom";

const prefetch = async (
	path: string[],
	parentId?: string,
): Promise<string | undefined> => {
	const where = {
		_and: parentId
			? [{ key: { _eq: path.at(0) } }, { parentId: { _eq: parentId } }]
			: [{ parentId: { _is_null: true } }],
	};
	const id = await resolve(({ query }) => {
		const node = query.nodes({ where }).at(0);
		node?.__typename;
		const id = node?.id;
		node?.name;

		return id;
	});

	await resolve(({ query }) => {
		if (id) {
			const node = query.node({ id: id });
			node?.name;
			node?.data?.({ path: "type" });
			node?.mimeId;
			node?.getIndex;
			node?.id;
		}
	});

	return path.length > 1
		? await prefetch(parentId ? path.slice(1) : path, id)
		: id;
};

const useLink = () => {
	const navigate = useNavigate();
	const pathname = usePath();

	const path = async (path: string[], app?: string) => {
		await prefetch(path);
		const query = app ? `?app=${app}` : "";
		return navigate(`/${path.join("/")}${query}`);
	};

	const id = async (id: string, app?: string) => {
		const path = await fromId(id);
		await prefetch(path);
		const query = app ? `?app=${app}` : "";
		return navigate(`/${path.join("/")}${query}`);
	};

	const push = async (path: string[], app?: string) => {
		const pushPath = pathname.split("/").concat(path);
		await prefetch(pushPath);
		const query = app ? `?app=${app}` : "";
		return navigate(`/${pushPath.join("/")}${query}`);
	};

	const pop = () => {
		const pushPath = pathname.split("/").slice(0, -1).map(decodeURI);
		return navigate(`/${pushPath.join("/")}`);
	};

	const back = () => navigate(-1);

	return { path, id, push, pop, back };
};

export default useLink;
