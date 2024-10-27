import { GetApp } from '@mui/icons-material';
import { AutoButton } from 'comps';
import { toHtml } from 'core/document';
import { order_by, resolve } from 'gql';
import { Node } from 'hooks';
import HTMLtoDOCX from 'html-to-docx';
import { getLetter } from 'mime';
import { nhost } from 'nhost';

const DownloadButton = ({ node }: { node: Node }) => {
  const query = node.useQuery();
  const id = query?.id;
  const mimeId = query?.mimeId;

  const formatContent = async (id: string, level: number): Promise<string> => {
    if (!id) return '';
    const index =
      (
        await resolve(({ query }) =>
          query
            .node({ id })
            ?.parent?.children({
              where: {
                _and: [
                  { mutable: { _eq: false } },
                  { mimeId: { _in: ['vote/change', 'vote/policy'] } },
                ],
              },
              order_by: [{ index: order_by.asc }, { createdAt: order_by.asc }],
            })
            .map(({ id }) => ({ id }))
        )
      )?.findIndex((e) => e.id === id) ?? 0;

    const children = await resolve(({ query }) =>
      query
        .node({ id })
        ?.children({
          order_by: [{ index: order_by.asc }],
          where: {
            mimeId: {
              _in: [
                'vote/position',
                'vote/candidate',
                'vote/policy',
                'vote/change',
              ],
            },
          },
        })
        .map(({ id }) => id)
    );
    const members = ['wiki/event', 'wiki/group'].includes(mimeId!)
      ? []
      : await resolve(({ query }) =>
          query
            .node({ id })
            ?.members()
            ?.map((m) => m.name ?? m.user?.displayName)
            .join(', ')
        );
    const node = await resolve(({ query }) => {
      const node = query.node({ id });
      if (node)
        return {
          name: node.name,
          data: node.data(),
          mimeId: node.mimeId,
          context: node.mime?.context,
        };
    });

    const formatedMembers = members?.length
      ? `<i>Stillet af: ${members}</i>`
      : '';

    const prefix =
      node?.mimeId == 'vote/policy'
        ? `${getLetter(index)}: `
        : node?.mimeId == 'vote/change'
        ? `${index + 1}: `
        : '';
    return `<h${level}>${prefix}${
      node?.name
    }</h${level}>${formatedMembers}${toHtml(node?.data?.content)}${
      children
        ? '<br>' +
          (
            await Promise.all(
              children.map((id) => formatContent(id!, level + 1))
            )
          ).join('<br>')
        : ''
    }`;
  };

  const handleDownload = async () => {
    if (!id) return;

    if (mimeId == 'wiki/file') {
      const data = await resolve(({ query }) => query.node({ id })?.data());
      const { presignedUrl } = await nhost.storage.getPresignedUrl({
        fileId: data?.fileId,
      });
      
      const anchor = document.createElement('a');
      // eslint-disable-next-line functional/immutable-data
      anchor.href = presignedUrl?.url as string;
      document.body.appendChild(anchor);
      anchor.click();
      document.body.removeChild(anchor);

      return;
    }

    const html = await formatContent(id!, 1);

    const blob = await HTMLtoDOCX(html as string, '', {
      table: { row: { cantSplit: true } },
    });

    // Create and evoke link to file
    const blobUrl = URL.createObjectURL(blob as Blob);
    const link = document.createElement('a');
    // eslint-disable-next-line functional/immutable-data
    link.href = blobUrl;
    // eslint-disable-next-line functional/immutable-data
    link.download = `${query?.name}.docx`;
    document.body.appendChild(link);
    link.dispatchEvent(
      new MouseEvent('click', {
        bubbles: true,
        cancelable: true,
        view: window,
      })
    );
    document.body.removeChild(link);
  };

  return (
    <AutoButton
      key="download"
      text="Download"
      icon={<GetApp />}
      onClick={handleDownload}
    />
  );
};

export default DownloadButton;
