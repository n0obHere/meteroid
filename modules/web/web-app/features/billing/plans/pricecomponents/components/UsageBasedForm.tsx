import {
  Button,
  ComboboxFormField,
  Form,
  FormControl,
  FormField,
  FormItem,
  FormMessage,
  GenericFormField,
  Input,
  InputFormField,
  SelectFormField,
  SelectItem,
} from '@md/ui'
import { ColumnDef } from '@tanstack/react-table'
import { useAtom, useSetAtom } from 'jotai'
import { PlusIcon, XIcon } from 'lucide-react'
import { memo, useCallback, useEffect, useMemo, useState } from 'react'
import { useFieldArray, useWatch } from 'react-hook-form'
import { useNavigate } from 'react-router-dom'
import { match } from 'ts-pattern'

import { AccordionPanel } from '@/components/AccordionPanel'
import PriceInput, { UncontrolledPriceInput } from '@/components/form/PriceInput'
import { SimpleTable } from '@/components/table/SimpleTable'
import {
  componentFeeAtom,
  componentNameAtom,
  EditPriceComponentCard,
  FeeFormProps,
} from '@/features/billing/plans/pricecomponents/EditPriceComponentCard'
import { useCurrency } from '@/features/billing/plans/pricecomponents/utils'
import { Methods, useZodForm } from '@/hooks/useZodForm'
import { useQuery } from '@/lib/connectrpc'
import {
  Matrix,
  TieredAndVolumeRow,
  UsageFee,
  UsageFeeSchema,
  UsagePricingModelType,
} from '@/lib/schemas/plans'
import {
  getBillableMetric,
  listBillableMetrics,
} from '@/rpc/api/billablemetrics/v1/billablemetrics-BillableMetricsService_connectquery'
import { useTypedParams } from '@/utils/params'

// type UsagePricingModelType = "per_unit" | "tiered" | "volume" | "package"

const models: [UsagePricingModelType, string][] = [
  ['per_unit', 'Per unit'],
  ['tiered', 'Tiered'],
  ['volume', 'Volume'],
  ['package', 'Package'],
  ['matrix', 'Matrix'],
]

const MetricSetter = ({
  methods,
  metricsOptions,
}: {
  methods: Methods<typeof UsageFeeSchema>
  metricsOptions: {
    label: string
    value: string
  }[]
}) => {
  const metricId = useWatch({
    control: methods.control,
    name: 'metricId',
  })

  const setName = useSetAtom(componentNameAtom)

  useEffect(() => {
    const metric = metricsOptions.find(m => m.value === metricId)
    metric?.label && setName(metric.label)
  }, [setName, metricId, metricsOptions])

  return null
}

export const UsageBasedForm = (props: FeeFormProps) => {
  const [component] = useAtom(componentFeeAtom)
  const navigate = useNavigate()

  const methods = useZodForm({
    schema: UsageFeeSchema,
    defaultValues: component?.data as UsageFee,
  })

  const { familyExternalId } = useTypedParams<{ familyExternalId: string }>()

  const metrics = useQuery(
    listBillableMetrics,
    {
      familyExternalId: familyExternalId!,
    },
    {
      enabled: !!familyExternalId,
    }
  )

  const metricsOptions =
    metrics.data?.billableMetrics?.map(metric => ({ label: metric.name, value: metric.id })) ?? []

  console.log('errors', methods.formState.errors)
  console.log('values', methods.getValues())

  return (
    <>
      <Form {...methods}>
        <MetricSetter methods={methods} metricsOptions={metricsOptions} />
        <EditPriceComponentCard submit={methods.handleSubmit(props.onSubmit)} cancel={props.cancel}>
          <div className="grid grid-cols-3 gap-2">
            <div className="col-span-1 pr-5 border-r border-border space-y-4">
              <ComboboxFormField
                name="metricId"
                label="Billable metric"
                control={methods.control}
                placeholder="Select a metric"
                options={metricsOptions}
                // empty={!metricsOptions.length}
                action={
                  <Button
                    hasIcon
                    variant="ghost"
                    size="full"
                    onClick={() => navigate('add-metric')}
                  >
                    <PlusIcon size={12} /> New metric
                  </Button>
                }
              />
            </div>
            <div className="ml-4 col-span-2 space-y-4">
              <SelectFormField
                name="model.model"
                label="Pricing model"
                placeholder="Select a model"
                className="max-w-[320px]"
                empty={models.length === 0}
                control={methods.control}
              >
                {models.map(([option, label]) => (
                  <SelectItem value={option} key={option}>
                    {label}
                  </SelectItem>
                ))}
              </SelectFormField>
              <UsageBasedDataForm methods={methods} />
            </div>
          </div>
        </EditPriceComponentCard>
      </Form>
    </>
  )
}

const UsageBasedDataForm = ({
  methods,
}: {
  methods: Methods<typeof UsageFeeSchema> // TODO
}) => {
  const model = useWatch({
    control: methods.control,
    name: 'model.model',
  })

  return match(model)
    .with('matrix', () => <MatrixForm methods={methods} />)
    .with('per_unit', () => <PerUnitForm methods={methods} />)
    .with('tiered', () => <TieredForm methods={methods} />)
    .with('volume', () => <VolumeForm methods={methods} />)
    .with('package', () => <PackageForm methods={methods} />)
    .exhaustive()
}

const MatrixForm = ({ methods }: { methods: Methods<typeof UsageFeeSchema> }) => {
  const currency = useCurrency()

  const { fields, append, remove } = useFieldArray({
    control: methods.control,
    name: 'model.data.dimensionRates',
  })

  const [dimensionHeaders, setDimensionHeaders] = useState<string[]>([])

  const metricId = useWatch({
    control: methods.control,
    name: 'metricId',
  })

  const metric = useQuery(getBillableMetric, { id: metricId }, { enabled: !!metricId })?.data

  useEffect(() => {
    if (!metric?.billableMetric?.segmentationMatrix) return

    const segmentationMatrix = metric.billableMetric.segmentationMatrix
    let headers: string[] = []
    let dimensionCombinations: Map<string, string>[] = []

    match(segmentationMatrix.matrix)
      .with({ case: 'single' }, ({ value }) => {
        headers = [value.dimension?.key ?? '']
        dimensionCombinations = (value.dimension?.values ?? []).map(v => new Map([[headers[0], v]]))
      })
      .with({ case: 'double' }, ({ value }) => {
        headers = [value.dimension1?.key ?? '', value.dimension2?.key ?? '']
        dimensionCombinations = (value.dimension1?.values ?? []).flatMap(v1 =>
          (value.dimension2?.values ?? []).map(
            v2 =>
              new Map([
                [headers[0], v1],
                [headers[1], v2],
              ])
          )
        )
      })
      .with({ case: 'linked' }, ({ value }) => {
        headers = [value.dimensionKey, value.linkedDimensionKey]
        dimensionCombinations = Object.entries(value.values).flatMap(([k, v]) =>
          v.values.map(
            linkedV =>
              new Map([
                [headers[0], k],
                [headers[1], linkedV],
              ])
          )
        )
      })
      .otherwise(() => {})

    setDimensionHeaders(headers)

    // Update or create rows based on the current state
    const currentDimensions = fields.map(field => field.dimensions)
    const newRows = dimensionCombinations.filter(
      combo =>
        !currentDimensions.some(dim =>
          Array.from(combo.entries()).every(([key, value]) => dim.get(key) === value)
        )
    )
    const removedRows = currentDimensions.filter(
      dim =>
        !dimensionCombinations.some(combo =>
          Array.from(combo.entries()).every(([key, value]) => dim.get(key) === value)
        )
    )

    newRows.forEach(dimensions => {
      append({ dimensions, price: '0' })
    })

    removedRows.forEach(dimensions => {
      const index = fields.findIndex(field =>
        Array.from(dimensions.entries()).every(
          ([key, value]) => field.dimensions.get(key) === value
        )
      )
      if (index !== -1) remove(index)
    })
  }, [metric, fields, append, remove])

  const columns = useMemo<ColumnDef<Matrix['dimensionRates'][number]>[]>(
    () => [
      ...dimensionHeaders.map(
        header =>
          ({
            header,
            accessorFn: row => row.dimensions.get(header),
          }) as ColumnDef<Matrix['dimensionRates'][number]>
      ),
      {
        header: 'Unit price',
        accessor: 'price',
        cell: ({ row }) => (
          <PriceInput
            {...methods.withControl(`model.data.dimensionRates.${row.index}.price`)}
            {...methods.withError(`model.data.dimensionRates.${row.index}.price`)}
            currency={currency}
            showCurrency={true}
            precision={8}
          />
        ),
      },
    ],
    [dimensionHeaders, methods, currency]
  )

  if (!metric?.billableMetric) return null

  const segmentationMatrix = metric.billableMetric.segmentationMatrix

  if (!segmentationMatrix) {
    return (
      <div className="py-4 text-sm text-muted-foreground">
        This metric does not have a segmentation matrix
      </div>
    )
  }

  return (
    <>
      <SimpleTable columns={columns} data={fields} />
    </>
  )
}

const PerUnitForm = ({
  methods,
}: {
  methods: Methods<typeof UsageFeeSchema> // TODO
}) => {
  const currency = useCurrency()

  return (
    <>
      <GenericFormField
        control={methods.control}
        name="model.data.unitPrice"
        label="Price per unit"
        render={({ field }) => (
          <UncontrolledPriceInput
            {...field}
            currency={currency}
            className="max-w-xs"
            precision={8}
          />
        )}
      />

      <div className="w-full border-b border-border pt-4"></div>

      <AccordionPanel
        title={
          <div className="space-x-4 items-center flex pr-4 text-muted-foreground">Adjustments</div>
        }
        defaultOpen={false}
        triggerClassName="justify-normal"
      >
        <div className="space-y-6">Included</div>
      </AccordionPanel>
    </>
  )
}

const TieredForm = ({
  methods,
}: {
  methods: Methods<typeof UsageFeeSchema> // TODO
}) => {
  const currency = useCurrency()
  return <TierTable methods={methods} currency={currency} />
}

const VolumeForm = ({
  methods,
}: {
  methods: Methods<typeof UsageFeeSchema> // TODO
}) => {
  const currency = useCurrency()
  return <TierTable methods={methods} currency={currency} />
}

const PackageForm = ({
  methods,
}: {
  methods: Methods<typeof UsageFeeSchema> // TODO
}) => {
  const currency = useCurrency()
  return (
    <>
      <InputFormField
        name="model.data.blockSize"
        label="Block size"
        type="number"
        step={1}
        className="max-w-xs"
        control={methods.control}
      />

      <GenericFormField
        control={methods.control}
        name="model.data.packagePrice"
        label="Price per block"
        render={({ field }) => (
          <UncontrolledPriceInput
            {...field}
            currency={currency}
            className="max-w-xs"
            precision={8}
          />
        )}
      />
    </>
  )
}

const TierTable = ({
  methods,
  currency,
}: {
  methods: Methods<typeof UsageFeeSchema>
  currency: string
}) => {
  const { fields, append, remove } = useFieldArray({
    control: methods.control,
    name: 'model.data.rows',
  })

  const [shouldInitTiers, setShouldInitTiers] = useState(false)

  useEffect(() => {
    if (fields.length === 0) {
      setShouldInitTiers(true)
    }
  }, [fields.length])

  useEffect(() => {
    if (shouldInitTiers) {
      append({ firstUnit: BigInt(0), unitPrice: '' })
      append({ firstUnit: BigInt(100), unitPrice: '' })
      setShouldInitTiers(false)
    }
  }, [shouldInitTiers, append])

  const addTier = useCallback(() => {
    const lastTier = fields[fields.length - 1]
    const firstUnit = lastTier ? BigInt(lastTier.firstUnit) + BigInt(2) : BigInt(0)
    append({ firstUnit, unitPrice: '' })
  }, [fields, append])

  const columns = useMemo<ColumnDef<TieredAndVolumeRow>[]>(
    () => [
      {
        header: 'First unit',
        cell: ({ row }) => <FirstUnitField methods={methods} rowIndex={row.index} />,
      },
      {
        header: 'Last unit',
        cell: ({ row }) => <LastUnitCell methods={methods} rowIndex={row.index} />,
      },
      {
        header: 'Per unit',
        cell: ({ row }) => (
          <GenericFormField
            control={methods.control}
            name={`model.data.rows.${row.index}.unitPrice`}
            render={({ field }) => (
              <UncontrolledPriceInput
                {...field}
                currency={currency}
                showCurrency={false}
                className="max-w-xs"
                precision={8}
              />
            )}
          />
        ),
      },
      {
        header: '',
        id: 'remove',
        cell: ({ row }) =>
          fields.length <= 2 || row.index === 0 ? null : (
            <Button
              variant="link"
              size="icon"
              onClick={() => remove(row.index)}
              disabled={fields.length <= 2}
            >
              <XIcon size={12} />
            </Button>
          ),
      },
    ],
    [methods, currency, fields.length, remove]
  )

  return (
    <>
      <SimpleTable columns={columns} data={fields} />
      <Button variant="link" onClick={addTier}>
        + Add tier
      </Button>
    </>
  )
}

const FirstUnitField = memo(
  ({ methods, rowIndex }: { methods: Methods<typeof UsageFeeSchema>; rowIndex: number }) => {
    const { control } = methods
    const prevRowValue = useWatch({
      control,
      name: `model.data.rows.${rowIndex - 1}`,
    })

    const isFirst = rowIndex === 0

    return (
      <>
        <FormField
          control={control}
          name={`model.data.rows.${rowIndex}.firstUnit`}
          rules={{
            min: isFirst ? 0 : Number(prevRowValue?.firstUnit ?? 0) + 2,
          }}
          render={({ field }) => (
            <FormItem>
              <FormControl>
                <Input
                  {...field}
                  type="number"
                  min={isFirst ? 0 : Number(prevRowValue?.firstUnit ?? 0) + 2}
                  onChange={e => {
                    const value = e.target.value
                    field.onChange(BigInt(value))
                  }}
                  value={Number(field.value)}
                  disabled={isFirst}
                  placeholder={isFirst ? '0' : ''}
                />
              </FormControl>
              <FormMessage />
            </FormItem>
          )}
        />
      </>
    )
  }
)

const LastUnitCell = ({
  methods,
  rowIndex,
}: {
  methods: Methods<typeof UsageFeeSchema>
  rowIndex: number
}) => {
  const nextRow = useWatch({
    control: methods.control,
    name: `model.data.rows.${rowIndex + 1}`,
  })

  const isLast = !nextRow

  return isLast ? '∞' : `${BigInt(nextRow.firstUnit) - BigInt(1)}`
}
