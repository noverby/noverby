import { nhost } from 'nhost';
import { startTransition, useEffect, useState } from 'react';

const useFiles = ({
  fileIds,
  quality,
  width = 500,
  height,
  image,
}: {
  fileIds?: string[];
  quality?: number;
  width?: number;
  height?: number;
  image?: boolean;
}) => {
  const [files, setFiles] = useState<string[]>([]);

  useEffect(() => {
    setFiles([]);
    const fetch = async () => {
      if (fileIds) {
        const preUrls = await Promise.all(
          fileIds.map((fileId) =>
            fileId
              ? nhost.storage.getPublicUrl({
                  fileId,
                  ...(image
                    ? {
                        quality,
                        width,
                        height,
                      }
                    : {}),
                })
              : Promise.resolve(null)
          )
        );
        setFiles(preUrls.map((preUrl) => preUrl ?? ''));
      }
    };
    startTransition(() => {
      fetch();
    });
  }, [JSON.stringify(fileIds)]);

  return files;
};

export default useFiles;
