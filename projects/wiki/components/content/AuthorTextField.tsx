import React, { startTransition, useEffect, useState } from 'react';
import { Box, Chip, TextField } from '@mui/material';
import { Autocomplete } from '@mui/material';
import { resolve, order_by } from 'gql';
import { IconId } from 'mime';
import { Face } from '@mui/icons-material';

const capitalize = (sentence: string) =>
  sentence.replace(/(^\w{1})|(\s+\w{1})/g, (letter) => letter.toUpperCase());

type Option = { name?: string; mimeId?: string; nodeId?: string };

const AuthorTextField = ({
  value,
  onChange,
  authorError,
  setAuthorError
}: {
  value: Option[];
  onChange: Function;
  authorError?: string;
  setAuthorError: Function;
}) => {
  const [options, setOptions] = useState<Option[]>([]);
  const [inputValue, setInputValue] = useState('');

  useEffect(() => {
    const fetch = async () => {
      const like = `%${inputValue}%`;

      const nodes = await resolve(({ query }) =>
        query
          .nodes({
            limit: 10,
            where: {
              _and: [
                { mimeId: { _eq: 'wiki/group' } },
                { name: { _ilike: like } },
              ],
            },
            order_by: [{ name: order_by.asc }],
          })
          .map(({ id, name }) => ({ name, nodeId: id, mimeId: 'wiki/group' }))
      );
      const users = await resolve(({ query }) =>
        query
          .users({
            limit: 10,
            order_by: [{ displayName: order_by.asc }],
            where: { displayName: { _ilike: like } }
          })
          .map(({ id, displayName }) => ({
            name: displayName,
            nodeId: id,
          }))
      );
      

      const newOptions = ([] as Option[]).concat(
        users && inputValue.length > 0 ? users : [],
        nodes && inputValue.length > 0 ? nodes : [],
        value ? value : [],
        inputValue ? [{ name: capitalize(inputValue) }] : []
      );

      setOptions(newOptions);
    };
    startTransition(() => {
      fetch();
    })
  }, [JSON.stringify(value), inputValue]);

  return (
    <Autocomplete
      multiple
      color="primary"
      noOptionsText="Ingen match"
      options={options}
      getOptionLabel={(option) => option?.name ?? ''}
      //defaultValue={options}
      value={value}
      filterSelectedOptions
      includeInputInList
      autoComplete
      autoHighlight
      fullWidth
      onChange={(_, newValue) => {
        setOptions(newValue.concat(options));
        onChange(newValue);
      }}
      onInputChange={(_, newInputValue) => {
        setAuthorError()
        setInputValue(newInputValue);
      }}
      renderOption={(props, option) => (
        <Box component="li" {...props}>
          <Chip
            variant="outlined"
            color="secondary"
            icon={option?.mimeId ? <IconId mimeId={option.mimeId} /> : <Face />}
            label={option?.name}
          />
        </Box>
      )}
      renderTags={(value, getCustomizedTagProps) =>
        value.map((option, index) => (
          <Chip
            variant="outlined"
            color="secondary"
            icon={option?.mimeId ? <IconId mimeId={option.mimeId} /> : <Face />}
            label={option?.name}
            {...getCustomizedTagProps({ index })}
            key={index}
          />
        ))
      }
      renderInput={(params) => (
        <TextField
          {...params}
          color="primary"
          variant="outlined"
          label="Forfattere"
          placeholder="Tilføj Forfatter"
          error={!!authorError}
          helperText={authorError}
        />
      )}
    />
  );
};

export default AuthorTextField;
